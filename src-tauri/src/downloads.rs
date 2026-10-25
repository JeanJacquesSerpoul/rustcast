//! Téléchargement des épisodes en tâche de fond, avec progression et annulation.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use shared::{DownloadProgress, DownloadStatus, Episode, EVT_DOWNLOAD, EVT_LIBRARY};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncWriteExt;

use crate::store::Library;
use crate::AppState;

const AUDIO_EXTS: &[&str] = &["mp3", "m4a", "aac", "ogg", "oga", "opus", "mp4", "m4v", "wav", "flac", "mov", "webm"];

fn extension(url: &str, mime: Option<&str>) -> &'static str {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    if let Some(ext) = path.rsplit('/').next().and_then(|f| f.rsplit_once('.')).map(|(_, e)| e.to_ascii_lowercase()) {
        if let Some(found) = AUDIO_EXTS.iter().find(|&&e| e == ext) {
            return found;
        }
    }
    match mime.unwrap_or_default() {
        m if m.contains("mp4") || m.contains("m4a") || m.contains("aac") => "m4a",
        m if m.contains("ogg") => "ogg",
        m if m.contains("opus") => "opus",
        m if m.contains("wav") => "wav",
        m if m.contains("flac") => "flac",
        _ => "mp3",
    }
}

const MAX_NAME_CHARS: usize = 90;

/// Transforme un titre en nom de fichier valide sous Windows, macOS et Linux.
pub fn sanitize_name(s: &str, fallback: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        let between_digits = i > 0
            && chars[i - 1].is_ascii_digit()
            && chars.get(i + 1).is_some_and(|n| n.is_ascii_digit());
        match c {
            // « 6/6 » ou « 12/03 » restent compacts.
            '/' if between_digits => out.push('-'),
            ':' | '/' | '\\' | '|' => out.push_str(" - "),
            '"' => out.push('\''),
            '<' | '>' | '?' | '*' => {}
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    let collapsed = out.split_whitespace().collect::<Vec<_>>().join(" ");
    // « A - - B » apparaît quand un séparateur suit un tiret existant.
    let mut name = collapsed.replace("- -", "-");
    while name.contains("  ") {
        name = name.replace("  ", " ");
    }
    let mut name: String = name.chars().take(MAX_NAME_CHARS).collect();
    // Windows refuse les noms terminés par un point ou une espace.
    while name.ends_with(['.', ' ', '-']) {
        name.pop();
    }
    let name = name.trim_start_matches(['.', ' ', '-']).to_string();
    if name.is_empty() {
        return fallback.to_string();
    }
    const RESERVED: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1",
        "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    if RESERVED.contains(&stem.as_str()) {
        format!("_{name}")
    } else {
        name
    }
}

/// Chemin lisible d'un épisode : `<podcast>/<AAAA-MM-JJ> - <titre>.<ext>`.
pub fn target_path(downloads_dir: &Path, podcast_title: &str, ep: &Episode) -> PathBuf {
    let folder = sanitize_name(podcast_title, &ep.podcast_id);
    let title = sanitize_name(&ep.title, &ep.id);
    let date = chrono::DateTime::from_timestamp(ep.pub_date, 0)
        .filter(|_| ep.pub_date > 0)
        .map(|d| d.with_timezone(&chrono::Local).format("%Y-%m-%d").to_string());
    let stem = match date {
        Some(d) => format!("{d} - {title}"),
        None => title,
    };
    downloads_dir
        .join(folder)
        .join(format!("{stem}.{}", extension(&ep.audio_url, ep.mime.as_deref())))
}

/// Ajoute un court identifiant si un autre fichier porte déjà ce nom.
fn unique_path(base: &Path, episode_id: &str) -> PathBuf {
    if !base.exists() && !base.with_extension("part").exists() {
        return base.to_path_buf();
    }
    let stem = base.file_stem().unwrap_or_default().to_string_lossy();
    let ext = base.extension().unwrap_or_default().to_string_lossy();
    base.with_file_name(format!("{stem} ({}).{ext}", &episode_id[..6.min(episode_id.len())]))
}

/// Supprime un dossier de podcast devenu vide.
pub fn remove_empty_parent(file: &Path) {
    if let Some(dir) = file.parent() {
        let _ = std::fs::remove_dir(dir);
    }
}

/// Renomme les téléchargements des anciennes versions (`<id podcast>/<id épisode>.mp3`)
/// avec des noms lisibles. Renvoie le nombre de fichiers renommés.
pub fn migrate_names(lib: &mut Library, downloads_dir: &Path) -> usize {
    let titles: HashMap<String, String> = lib.podcasts.iter().map(|p| (p.id.clone(), p.title.clone())).collect();
    let mut renamed = 0;
    for ep in lib.episodes.values_mut().flatten() {
        let Some(current) = ep.local_path.as_ref().map(PathBuf::from) else {
            continue;
        };
        let is_legacy = current.file_stem().is_some_and(|s| s.to_string_lossy() == ep.id);
        if !is_legacy || !current.exists() {
            continue;
        }
        let title = titles.get(&ep.podcast_id).map(String::as_str).unwrap_or_default();
        // On garde l'extension réelle du fichier avant de vérifier que le nom est libre.
        let mut base = target_path(downloads_dir, title, ep);
        if let Some(ext) = current.extension() {
            base.set_extension(ext);
        }
        let target = unique_path(&base, &ep.id);
        if std::fs::create_dir_all(target.parent().unwrap()).is_err() || std::fs::rename(&current, &target).is_err() {
            continue;
        }
        remove_empty_parent(&current);
        ep.local_path = Some(target.to_string_lossy().to_string());
        renamed += 1;
    }
    renamed
}

fn emit(app: &AppHandle, p: &DownloadProgress) {
    let _ = app.emit(EVT_DOWNLOAD, p);
}

fn set_status(app: &AppHandle, id: &str, f: impl FnOnce(&mut DownloadProgress)) -> Option<DownloadProgress> {
    let state = app.state::<AppState>();
    let mut map = state.downloads.lock().unwrap();
    let (p, _) = map.get_mut(id)?;
    f(p);
    Some(p.clone())
}

pub fn start(app: &AppHandle, episode_id: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    let ep = state
        .store
        .with(|l| l.episode(episode_id).cloned())
        .ok_or("Épisode introuvable")?;
    if ep.local_path.is_some() {
        return Ok(());
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let progress = DownloadProgress {
        episode_id: ep.id.clone(),
        podcast_id: ep.podcast_id.clone(),
        title: ep.title.clone(),
        downloaded: 0,
        total: ep.size,
        status: DownloadStatus::Queued,
    };
    {
        let mut map = state.downloads.lock().unwrap();
        if let Some((p, _)) = map.get(&ep.id) {
            if matches!(p.status, DownloadStatus::Queued | DownloadStatus::Downloading) {
                return Ok(());
            }
        }
        map.insert(ep.id.clone(), (progress.clone(), cancel.clone()));
    }
    emit(app, &progress);

    let app = app.clone();
    let podcast_title = state
        .store
        .with(|l| l.podcast(&ep.podcast_id).map(|p| p.title.clone()))
        .unwrap_or_default();
    let base = target_path(&state.store.downloads_dir(), &podcast_title, &ep);
    let sem = state.dl_sem.clone();
    tauri::async_runtime::spawn(async move {
        let _permit = sem.acquire_owned().await;
        if cancel.load(Ordering::Acquire) {
            return;
        }
        let file = unique_path(&base, &ep.id);
        let result = fetch(&app, &ep.id, &ep.audio_url, &file, &cancel).await;
        let final_state = match result {
            Ok(size) => {
                let path = file.to_string_lossy().to_string();
                app.state::<AppState>().store.update(|l| {
                    if let Some(e) = l.episode_mut(&ep.id) {
                        e.local_path = Some(path);
                        e.local_size = Some(size);
                    }
                });
                let _ = app.emit(EVT_LIBRARY, ());
                DownloadStatus::Done
            }
            Err(_) if cancel.load(Ordering::Acquire) => DownloadStatus::Cancelled,
            Err(e) => DownloadStatus::Failed(e),
        };
        if let Some(p) = set_status(&app, &ep.id, |p| p.status = final_state) {
            emit(&app, &p);
        }
    });
    Ok(())
}

async fn fetch(app: &AppHandle, id: &str, url: &str, dest: &Path, cancel: &AtomicBool) -> Result<u64, String> {
    tokio::fs::create_dir_all(dest.parent().unwrap())
        .await
        .map_err(|e| e.to_string())?;
    let client = app.state::<AppState>().dl_client.clone();
    let resp = client
        .get(url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?;
    let total = resp.content_length();
    if let Some(p) = set_status(app, id, |p| {
        p.status = DownloadStatus::Downloading;
        p.total = total.or(p.total);
    }) {
        emit(app, &p);
    }

    let part: PathBuf = dest.with_extension("part");
    let mut out = tokio::fs::File::create(&part).await.map_err(|e| e.to_string())?;
    let mut stream = resp.bytes_stream();
    let mut done: u64 = 0;
    let mut last_emit = Instant::now();
    let result: Result<(), String> = async {
        while let Some(chunk) = stream.next().await {
            if cancel.load(Ordering::Acquire) {
                return Err("annulé".into());
            }
            let chunk = chunk.map_err(|e| e.to_string())?;
            out.write_all(&chunk).await.map_err(|e| e.to_string())?;
            done += chunk.len() as u64;
            if last_emit.elapsed() > Duration::from_millis(200) {
                last_emit = Instant::now();
                if let Some(p) = set_status(app, id, |p| p.downloaded = done) {
                    emit(app, &p);
                }
            }
        }
        out.flush().await.map_err(|e| e.to_string())
    }
    .await;
    drop(out);
    if let Err(e) = result {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(e);
    }
    tokio::fs::rename(&part, dest).await.map_err(|e| e.to_string())?;
    Ok(done)
}

pub fn cancel(app: &AppHandle, episode_id: &str) {
    let state = app.state::<AppState>();
    let mut map = state.downloads.lock().unwrap();
    if let Some((p, flag)) = map.get_mut(episode_id) {
        flag.store(true, Ordering::Release);
        p.status = DownloadStatus::Cancelled;
        emit(app, p);
    }
    map.remove(episode_id);
}

/// Supprime le fichier local d'un épisode. Renvoie vrai si un fichier existait.
pub fn delete_local(app: &AppHandle, episode_id: &str) -> bool {
    let state = app.state::<AppState>();
    let path = state.store.update(|l| {
        l.episode_mut(episode_id).and_then(|e| {
            e.local_size = None;
            e.local_path.take()
        })
    });
    state.downloads.lock().unwrap().remove(episode_id);
    match path {
        Some(p) => {
            let _ = std::fs::remove_file(&p);
            remove_empty_parent(Path::new(&p));
            true
        }
        None => false,
    }
}

pub fn dir_size(dir: &Path) -> (u64, usize) {
    let mut total = (0, 0);
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let (b, n) = dir_size(&path);
                total.0 += b;
                total.1 += n;
            } else if let Ok(meta) = entry.metadata() {
                total.0 += meta.len();
                total.1 += 1;
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    fn episode(title: &str, pub_date: i64) -> Episode {
        Episode {
            id: "abcdef0123456789".into(),
            podcast_id: "p1".into(),
            title: title.into(),
            pub_date,
            audio_url: "https://ex.com/file.mp3?x=1".into(),
            ..Default::default()
        }
    }

    #[test]
    fn sanitizes_names() {
        assert_eq!(sanitize_name("Ép. 12: L'été / la nuit?", "x"), "Ép. 12 - L'été - la nuit");
        assert_eq!(sanitize_name("  <<Titre>> \"cité\" *** ...", "x"), "Titre 'cité'");
        assert_eq!(sanitize_name("Partie 1 - : suite", "x"), "Partie 1 - suite");
        assert_eq!(sanitize_name("con", "x"), "_con");
        assert_eq!(sanitize_name("???", "repli"), "repli");
        assert_eq!(sanitize_name(&"a".repeat(200), "x").chars().count(), MAX_NAME_CHARS);
        assert_eq!(sanitize_name("Ligne\nretour\tTab", "x"), "Ligne retour Tab");
        assert_eq!(
            sanitize_name("L'art de l'interview 6/6 : L'inconnu", "x"),
            "L'art de l'interview 6-6 - L'inconnu"
        );
    }

    #[test]
    fn readable_paths() {
        let dir = Path::new("dl");
        // Midi UTC : la même date quel que soit le fuseau du poste de test.
        let p = target_path(dir, "La Méthode : scientifique", &episode("Le vide, enfin ?", 1_700_049_600));
        assert_eq!(p, dir.join("La Méthode - scientifique").join("2023-11-15 - Le vide, enfin.mp3"));
        let undated = target_path(dir, "", &episode("Sans date", 0));
        assert_eq!(undated, dir.join("p1").join("Sans date.mp3"));
    }

    #[test]
    fn avoids_collisions_and_migrates_legacy_files() {
        let dir = std::env::temp_dir().join(format!("rustcast-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let legacy_dir = dir.join("p1");
        std::fs::create_dir_all(&legacy_dir).unwrap();
        let legacy = legacy_dir.join("abcdef0123456789.m4a");
        std::fs::write(&legacy, b"audio").unwrap();

        let mut ep = episode("Bonjour", 0);
        ep.local_path = Some(legacy.to_string_lossy().to_string());
        let mut lib = Library::default();
        lib.podcasts.push(shared::Podcast { id: "p1".into(), title: "Mon Podcast".into(), ..Default::default() });
        // Un autre fichier occupe déjà le nom visé.
        std::fs::create_dir_all(dir.join("Mon Podcast")).unwrap();
        std::fs::write(dir.join("Mon Podcast").join("Bonjour.m4a"), b"autre").unwrap();
        lib.episodes.insert("p1".into(), vec![ep]);

        assert_eq!(migrate_names(&mut lib, &dir), 1);
        let new_path = PathBuf::from(lib.episodes["p1"][0].local_path.clone().unwrap());
        assert_eq!(new_path, dir.join("Mon Podcast").join("Bonjour (abcdef).m4a"));
        assert_eq!(std::fs::read(&new_path).unwrap(), b"audio");
        assert!(!legacy_dir.exists(), "l'ancien dossier vide doit disparaître");
        // Une seconde passe ne fait rien.
        assert_eq!(migrate_names(&mut lib, &dir), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn extensions() {
        assert_eq!(super::extension("https://a.com/x/ep.M4A?tok=1", None), "m4a");
        assert_eq!(super::extension("https://a.com/stream", Some("audio/ogg")), "ogg");
        assert_eq!(super::extension("https://a.com/stream", None), "mp3");
    }
}
