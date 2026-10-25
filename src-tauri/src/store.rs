//! Persistance de la bibliothèque dans un fichier JSON du dossier de données.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use shared::{Episode, EpisodeView, Podcast, PodcastSummary, Settings};

#[derive(Default, Serialize, Deserialize)]
pub struct Library {
    pub podcasts: Vec<Podcast>,
    /// Épisodes par identifiant de podcast, triés du plus récent au plus ancien.
    pub episodes: HashMap<String, Vec<Episode>>,
    pub queue: Vec<String>,
    pub settings: Settings,
}

impl Library {
    pub fn load(path: &Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec(self)?)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn podcast(&self, id: &str) -> Option<&Podcast> {
        self.podcasts.iter().find(|p| p.id == id)
    }

    pub fn episode(&self, id: &str) -> Option<&Episode> {
        self.episodes.values().flatten().find(|e| e.id == id)
    }

    pub fn episode_mut(&mut self, id: &str) -> Option<&mut Episode> {
        self.episodes.values_mut().flatten().find(|e| e.id == id)
    }

    pub fn view(&self, ep: &Episode) -> EpisodeView {
        let podcast = self.podcast(&ep.podcast_id);
        EpisodeView {
            episode: ep.clone(),
            podcast_title: podcast.map(|p| p.title.clone()).unwrap_or_default(),
            podcast_artwork: podcast.and_then(|p| p.artwork.clone()),
        }
    }

    pub fn view_by_id(&self, id: &str) -> Option<EpisodeView> {
        self.episode(id).map(|e| self.view(e))
    }

    pub fn summary(&self, p: &Podcast) -> PodcastSummary {
        let eps = self.episodes.get(&p.id).map(Vec::as_slice).unwrap_or(&[]);
        PodcastSummary {
            podcast: p.clone(),
            episode_count: eps.len(),
            unplayed_count: eps.iter().filter(|e| !e.played).count(),
            downloaded_count: eps.iter().filter(|e| e.local_path.is_some()).count(),
            latest_pub: eps.iter().map(|e| e.pub_date).max().unwrap_or(0),
        }
    }

    /// Fusionne des épisodes fraîchement récupérés en conservant l'état local.
    /// Renvoie le nombre de nouveaux épisodes.
    pub fn merge_episodes(&mut self, podcast_id: &str, fresh: Vec<Episode>) -> usize {
        let old: HashMap<String, Episode> = self
            .episodes
            .remove(podcast_id)
            .unwrap_or_default()
            .into_iter()
            .map(|e| (e.id.clone(), e))
            .collect();
        let had_any = !old.is_empty();
        let mut new_count = 0;
        let mut merged: Vec<Episode> = fresh
            .into_iter()
            .map(|mut e| {
                if let Some(prev) = old.get(&e.id) {
                    e.position = prev.position;
                    e.played = prev.played;
                    e.local_path = prev.local_path.clone();
                    e.local_size = prev.local_size;
                } else if had_any {
                    new_count += 1;
                }
                e
            })
            .collect();
        // Les épisodes téléchargés qui ont disparu du flux sont conservés.
        let fresh_ids: std::collections::HashSet<String> =
            merged.iter().map(|e| e.id.clone()).collect();
        for (id, e) in old {
            if !fresh_ids.contains(&id) && e.local_path.is_some() {
                merged.push(e);
            }
        }
        merged.sort_by(|a, b| b.pub_date.cmp(&a.pub_date));
        self.episodes.insert(podcast_id.to_string(), merged);
        new_count
    }
}

impl Library {
    /// Réécrit les chemins des fichiers téléchargés après un déplacement du dossier de données.
    pub fn rebase_paths(&mut self, old: &Path, new: &Path) -> usize {
        let mut count = 0;
        for ep in self.episodes.values_mut().flatten() {
            let Some(path) = ep.local_path.as_ref().map(PathBuf::from) else {
                continue;
            };
            // Seuls les fichiers réellement déplacés changent de chemin.
            if let Ok(rest) = path.strip_prefix(old) {
                let moved = new.join(rest);
                if moved.exists() {
                    ep.local_path = Some(moved.to_string_lossy().to_string());
                    count += 1;
                }
            }
        }
        count
    }
}

/// Identifiant des premières versions, abandonné car le suffixe « .app » gêne sous macOS.
pub const LEGACY_ID: &str = "com.rustcast.app";

/// Déplace les données de l'ancien dossier vers le nouveau, sans rien écraser.
/// Renvoie l'ancien dossier si des données ont été déplacées.
pub fn migrate_data_dir(legacy: &Path, data_dir: &Path) -> Option<PathBuf> {
    if !legacy.is_dir() || data_dir.join("library.json").exists() {
        return None;
    }
    if !data_dir.exists() && std::fs::rename(legacy, data_dir).is_ok() {
        return Some(legacy.to_path_buf());
    }
    std::fs::create_dir_all(data_dir).ok()?;
    let mut moved = false;
    for entry in std::fs::read_dir(legacy).ok()?.flatten() {
        let target = data_dir.join(entry.file_name());
        if !target.exists() && std::fs::rename(entry.path(), &target).is_ok() {
            moved = true;
        }
    }
    let _ = std::fs::remove_dir(legacy);
    moved.then(|| legacy.to_path_buf())
}

/// Supprime le cache WebView de l'ancien identifiant, s'il ne contient rien d'autre.
pub fn remove_legacy_webview_cache(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let only_cache = entries.flatten().all(|e| e.file_name() == "EBWebView");
    if only_cache {
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// Bibliothèque partagée avec un indicateur de modification pour l'enregistrement différé.
pub struct Store {
    pub lib: std::sync::Mutex<Library>,
    pub path: PathBuf,
    pub data_dir: PathBuf,
    dirty: AtomicBool,
}

impl Store {
    pub fn open(data_dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&data_dir);
        let path = data_dir.join("library.json");
        Self {
            lib: std::sync::Mutex::new(Library::load(&path)),
            path,
            data_dir,
            dirty: AtomicBool::new(false),
        }
    }

    pub fn downloads_dir(&self) -> PathBuf {
        self.data_dir.join("downloads")
    }

    pub fn with<R>(&self, f: impl FnOnce(&Library) -> R) -> R {
        f(&self.lib.lock().unwrap())
    }

    /// Modifie la bibliothèque et marque l'état comme à enregistrer.
    pub fn update<R>(&self, f: impl FnOnce(&mut Library) -> R) -> R {
        let r = f(&mut self.lib.lock().unwrap());
        self.dirty.store(true, Ordering::Release);
        r
    }

    pub fn flush(&self) {
        if self.dirty.swap(false, Ordering::AcqRel) {
            let lib = self.lib.lock().unwrap();
            if let Err(e) = lib.save(&self.path) {
                eprintln!("Échec de l'enregistrement de la bibliothèque : {e}");
                self.dirty.store(true, Ordering::Release);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library_with_download(path: &Path) -> Library {
        let mut lib = Library::default();
        let ep = Episode {
            id: "e1".into(),
            local_path: Some(path.to_string_lossy().to_string()),
            ..Default::default()
        };
        lib.episodes.insert("p1".into(), vec![ep]);
        lib
    }

    fn setup(name: &str) -> (PathBuf, PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("rustcast-store-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let legacy = root.join(LEGACY_ID);
        let file = legacy.join("downloads").join("Pod").join("ep.mp3");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, b"audio").unwrap();
        library_with_download(&file).save(&legacy.join("library.json")).unwrap();
        (root.clone(), legacy, root.join("io.github.test.rustcast"))
    }

    fn check(legacy: &Path, data_dir: &Path, moved: Option<PathBuf>) {
        assert_eq!(moved.as_deref(), Some(legacy));
        let store = Store::open(data_dir.to_path_buf());
        assert_eq!(store.update(|l| l.rebase_paths(legacy, data_dir)), 1);
        let path = store.with(|l| l.episode("e1").unwrap().local_path.clone().unwrap());
        assert_eq!(PathBuf::from(&path), data_dir.join("downloads").join("Pod").join("ep.mp3"));
        assert_eq!(std::fs::read(path).unwrap(), b"audio");
        assert!(!legacy.exists());
        // Une seconde migration ne fait rien.
        assert_eq!(migrate_data_dir(legacy, data_dir), None);
    }

    #[test]
    fn rebase_keeps_files_that_did_not_move() {
        let (root, legacy, data_dir) = setup("partial");
        let old_file = legacy.join("downloads").join("Pod").join("ep.mp3");
        let mut lib = library_with_download(&old_file);
        // Le dossier downloads est resté à l'ancien emplacement (fichier verrouillé par exemple).
        std::fs::create_dir_all(&data_dir).unwrap();
        assert_eq!(lib.rebase_paths(&legacy, &data_dir), 0);
        assert_eq!(PathBuf::from(lib.episode("e1").unwrap().local_path.clone().unwrap()), old_file);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn moves_whole_legacy_dir() {
        let (root, legacy, data_dir) = setup("whole");
        let moved = migrate_data_dir(&legacy, &data_dir);
        check(&legacy, &data_dir, moved);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn merges_into_existing_dir() {
        let (root, legacy, data_dir) = setup("merge");
        // Le système a déjà créé le nouveau dossier (cache WebView par exemple).
        std::fs::create_dir_all(data_dir.join("EBWebView")).unwrap();
        let moved = migrate_data_dir(&legacy, &data_dir);
        check(&legacy, &data_dir, moved);
        assert!(data_dir.join("EBWebView").exists());
        let _ = std::fs::remove_dir_all(root);
    }
}
