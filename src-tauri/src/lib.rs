mod downloads;
mod feed;
mod itunes;
#[cfg(target_os = "linux")]
mod media_server;
mod opml;
mod store;

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use shared::*;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use tokio::sync::Semaphore;

use store::Store;

pub struct AppState {
    pub store: Arc<Store>,
    pub client: reqwest::Client,
    pub dl_client: reqwest::Client,
    pub downloads: Mutex<HashMap<String, (DownloadProgress, Arc<AtomicBool>)>>,
    pub dl_sem: Arc<Semaphore>,
    /// Flux consultés sans abonnement (page de découverte).
    pub previews: Mutex<HashMap<String, (Podcast, Vec<Episode>)>>,
}

type Res<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn notify_library(app: &AppHandle) {
    let _ = app.emit(EVT_LIBRARY, ());
}

// ───────────────────────────── Réglages ─────────────────────────────

#[tauri::command(rename_all = "snake_case")]
fn get_settings(state: State<AppState>) -> Settings {
    state.store.with(|l| l.settings.clone())
}

#[tauri::command(rename_all = "snake_case")]
fn save_settings(state: State<AppState>, settings: Settings) {
    state.store.update(|l| l.settings = settings);
}

// ─────────────────────────── Bibliothèque ───────────────────────────

#[tauri::command(rename_all = "snake_case")]
fn list_podcasts(state: State<AppState>) -> Vec<PodcastSummary> {
    state.store.with(|l| {
        let mut list: Vec<_> = l.podcasts.iter().map(|p| l.summary(p)).collect();
        list.sort_by(|a, b| b.latest_pub.cmp(&a.latest_pub));
        list
    })
}

#[tauri::command(rename_all = "snake_case")]
fn get_podcast(state: State<AppState>, id: String) -> Res<FeedPreview> {
    if let Some(found) = state.store.with(|l| {
        l.podcast(&id).map(|p| FeedPreview {
            podcast: p.clone(),
            episodes: l
                .episodes
                .get(&id)
                .map(|eps| eps.iter().map(|e| l.view(e)).collect())
                .unwrap_or_default(),
            subscribed: true,
        })
    }) {
        return Ok(found);
    }
    let previews = state.previews.lock().unwrap();
    let (p, eps) = previews.get(&id).ok_or("Podcast introuvable")?;
    Ok(preview_of(p, eps, false))
}

fn preview_of(p: &Podcast, eps: &[Episode], subscribed: bool) -> FeedPreview {
    FeedPreview {
        podcast: p.clone(),
        episodes: eps
            .iter()
            .map(|e| EpisodeView {
                episode: e.clone(),
                podcast_title: p.title.clone(),
                podcast_artwork: p.artwork.clone(),
            })
            .collect(),
        subscribed,
    }
}

/// Charge un flux pour le consulter avant de s'abonner.
#[tauri::command(rename_all = "snake_case")]
async fn preview_feed(state: State<'_, AppState>, feed_url: String) -> Res<FeedPreview> {
    let id = feed::podcast_id(&feed_url);
    if state.store.with(|l| l.podcast(&id).is_some()) {
        return get_podcast(state, id);
    }
    if let Some((p, eps)) = state.previews.lock().unwrap().get(&id) {
        return Ok(preview_of(p, eps, false));
    }
    let (p, eps) = feed::fetch(&state.client, &feed_url).await.map_err(err)?;
    let preview = preview_of(&p, &eps, false);
    state.previews.lock().unwrap().insert(p.id.clone(), (p, eps));
    Ok(preview)
}

async fn subscribe_inner(state: &AppState, feed_url: &str) -> Res<PodcastSummary> {
    let id = feed::podcast_id(feed_url);
    if let Some(s) = state.store.with(|l| l.podcast(&id).map(|p| l.summary(p))) {
        return Ok(s);
    }
    let cached = state.previews.lock().unwrap().remove(&id);
    let (podcast, episodes) = match cached {
        Some(c) => c,
        None => feed::fetch(&state.client, feed_url).await.map_err(err)?,
    };
    Ok(state.store.update(|l| {
        let id = podcast.id.clone();
        l.podcasts.push(podcast);
        l.merge_episodes(&id, episodes);
        l.summary(l.podcast(&id).unwrap())
    }))
}

#[tauri::command(rename_all = "snake_case")]
async fn subscribe(app: AppHandle, state: State<'_, AppState>, feed_url: String) -> Res<PodcastSummary> {
    let s = subscribe_inner(&state, &feed_url).await?;
    state.store.flush();
    notify_library(&app);
    Ok(s)
}

#[tauri::command(rename_all = "snake_case")]
fn unsubscribe(app: AppHandle, state: State<AppState>, id: String) {
    let eps: Vec<Episode> = state.store.update(|l| {
        l.podcasts.retain(|p| p.id != id);
        let eps = l.episodes.remove(&id).unwrap_or_default();
        l.queue.retain(|q| !eps.iter().any(|e| &e.id == q));
        eps
    });
    for ep in &eps {
        downloads::cancel(&app, &ep.id);
        // Le dossier porte le nom du podcast et peut être partagé avec un homonyme :
        // on ne supprime que les fichiers de ce podcast.
        if let Some(path) = &ep.local_path {
            let _ = std::fs::remove_file(path);
            downloads::remove_empty_parent(std::path::Path::new(path));
        }
    }
    // Dossier des anciennes versions, nommé d'après l'identifiant du podcast.
    let _ = std::fs::remove_dir_all(state.store.downloads_dir().join(&id));
    state.store.flush();
    notify_library(&app);
}

async fn refresh_one(state: &AppState, id: &str) -> Res<usize> {
    let url = state
        .store
        .with(|l| l.podcast(id).map(|p| p.feed_url.clone()))
        .ok_or("Podcast introuvable")?;
    let (fresh, episodes) = feed::fetch(&state.client, &url).await.map_err(err)?;
    Ok(state.store.update(|l| {
        if let Some(p) = l.podcasts.iter_mut().find(|p| p.id == id) {
            let added_at = p.added_at;
            *p = Podcast { added_at, ..fresh };
        }
        l.merge_episodes(id, episodes)
    }))
}

#[tauri::command(rename_all = "snake_case")]
async fn refresh_podcast(app: AppHandle, state: State<'_, AppState>, id: String) -> Res<usize> {
    let n = refresh_one(&state, &id).await?;
    notify_library(&app);
    Ok(n)
}

async fn refresh_all_inner(app: &AppHandle) -> usize {
    let state = app.state::<AppState>();
    let ids: Vec<String> = state.store.with(|l| l.podcasts.iter().map(|p| p.id.clone()).collect());
    let total: usize = futures_util::stream::iter(ids)
        .map(|id| {
            let state = app.state::<AppState>();
            async move { refresh_one(&state, &id).await.unwrap_or(0) }
        })
        .buffer_unordered(6)
        .fold(0, |acc, n| async move { acc + n })
        .await;
    notify_library(app);
    total
}

#[tauri::command(rename_all = "snake_case")]
async fn refresh_all(app: AppHandle) -> usize {
    refresh_all_inner(&app).await
}

// ───────────────────────────── Épisodes ─────────────────────────────

#[tauri::command(rename_all = "snake_case")]
fn get_episode(state: State<AppState>, id: String) -> Option<EpisodeView> {
    state.store.with(|l| l.view_by_id(&id))
}

#[tauri::command(rename_all = "snake_case")]
fn get_latest(state: State<AppState>, limit: usize) -> Vec<EpisodeView> {
    state.store.with(|l| {
        let mut all: Vec<&Episode> = l.episodes.values().flatten().collect();
        all.sort_by(|a, b| b.pub_date.cmp(&a.pub_date));
        all.into_iter().take(limit).map(|e| l.view(e)).collect()
    })
}

#[tauri::command(rename_all = "snake_case")]
fn get_in_progress(state: State<AppState>) -> Vec<EpisodeView> {
    state.store.with(|l| {
        let mut list: Vec<&Episode> = l
            .episodes
            .values()
            .flatten()
            .filter(|e| !e.played && e.position > 5.0)
            .collect();
        list.sort_by(|a, b| b.pub_date.cmp(&a.pub_date));
        list.into_iter().take(12).map(|e| l.view(e)).collect()
    })
}

#[tauri::command(rename_all = "snake_case")]
fn get_downloaded(state: State<AppState>) -> Vec<EpisodeView> {
    state.store.with(|l| {
        let mut list: Vec<&Episode> = l.episodes.values().flatten().filter(|e| e.local_path.is_some()).collect();
        list.sort_by(|a, b| b.pub_date.cmp(&a.pub_date));
        list.into_iter().map(|e| l.view(e)).collect()
    })
}

#[tauri::command(rename_all = "snake_case")]
fn set_progress(state: State<AppState>, id: String, position: f64, duration: Option<f64>) {
    state.store.update(|l| {
        if let Some(e) = l.episode_mut(&id) {
            e.position = position.max(0.0);
            if let Some(d) = duration.filter(|d| d.is_finite() && *d > 1.0) {
                if e.duration.map_or(true, |old| (old as f64 - d).abs() > 5.0) {
                    e.duration = Some(d as u32);
                }
            }
        }
        // Le dernier épisode écouté est enregistré par l'interface au lancement de la
        // lecture : le réécrire ici ferait revenir un épisode que l'on vient de fermer.
    });
}

#[tauri::command(rename_all = "snake_case")]
fn mark_played(app: AppHandle, state: State<AppState>, id: String, played: bool) {
    let auto_delete = state.store.update(|l| {
        if let Some(e) = l.episode_mut(&id) {
            e.played = played;
            e.position = 0.0;
        }
        if played {
            l.queue.retain(|q| q != &id);
        }
        l.settings.auto_delete_played
    });
    if played && auto_delete {
        downloads::delete_local(&app, &id);
    }
    notify_library(&app);
}

#[tauri::command(rename_all = "snake_case")]
fn mark_all_played(app: AppHandle, state: State<AppState>, podcast_id: String) {
    state.store.update(|l| {
        if let Some(eps) = l.episodes.get_mut(&podcast_id) {
            for e in eps {
                e.played = true;
                e.position = 0.0;
            }
        }
    });
    notify_library(&app);
}

// ─────────────────────────── File d'attente ──────────────────────────

#[tauri::command(rename_all = "snake_case")]
fn get_queue(state: State<AppState>) -> Vec<EpisodeView> {
    state.store.with(|l| l.queue.iter().filter_map(|id| l.view_by_id(id)).collect())
}

#[tauri::command(rename_all = "snake_case")]
fn set_queue(app: AppHandle, state: State<AppState>, ids: Vec<String>) {
    state.store.update(|l| l.queue = ids);
    notify_library(&app);
}

#[tauri::command(rename_all = "snake_case")]
fn queue_add(app: AppHandle, state: State<AppState>, id: String, next: bool) {
    state.store.update(|l| {
        l.queue.retain(|q| q != &id);
        if next {
            l.queue.insert(0, id);
        } else {
            l.queue.push(id);
        }
    });
    notify_library(&app);
}

#[tauri::command(rename_all = "snake_case")]
fn queue_remove(app: AppHandle, state: State<AppState>, id: String) {
    state.store.update(|l| l.queue.retain(|q| q != &id));
    notify_library(&app);
}

// ──────────────────────────── Découverte ────────────────────────────

#[tauri::command(rename_all = "snake_case")]
async fn search_podcasts(state: State<'_, AppState>, term: String, country: String) -> Res<Vec<SearchResult>> {
    let term = term.trim();
    if term.contains("://") {
        // Une URL de flux collée directement dans la recherche.
        let preview = preview_feed(state, term.to_string()).await?;
        return Ok(vec![SearchResult {
            collection_id: 0,
            title: preview.podcast.title,
            author: preview.podcast.author,
            artwork: preview.podcast.artwork,
            feed_url: Some(preview.podcast.feed_url),
            genre: preview.podcast.categories.first().cloned(),
            episode_count: Some(preview.episodes.len() as u32),
        }]);
    }
    itunes::search(&state.client, term, &country).await.map_err(err)
}

#[tauri::command(rename_all = "snake_case")]
async fn top_podcasts(state: State<'_, AppState>, country: String) -> Res<Vec<SearchResult>> {
    itunes::top(&state.client, &country).await.map_err(err)
}

// ─────────────────────────── Téléchargements ─────────────────────────

#[tauri::command(rename_all = "snake_case")]
fn download_episode(app: AppHandle, id: String) -> Res<()> {
    downloads::start(&app, &id)
}

#[tauri::command(rename_all = "snake_case")]
fn cancel_download(app: AppHandle, id: String) {
    downloads::cancel(&app, &id);
}

#[tauri::command(rename_all = "snake_case")]
fn delete_download(app: AppHandle, id: String) {
    downloads::delete_local(&app, &id);
    notify_library(&app);
}

/// Ouvre l'explorateur de fichiers sur le fichier téléchargé d'un épisode.
#[tauri::command(rename_all = "snake_case")]
fn reveal_download(app: AppHandle, state: State<AppState>, id: String) -> Res<()> {
    let path = state
        .store
        .with(|l| l.episode(&id).and_then(|e| e.local_path.clone()))
        .ok_or("Épisode non téléchargé")?;
    app.opener().reveal_item_in_dir(path).map_err(err)
}

#[tauri::command(rename_all = "snake_case")]
fn active_downloads(state: State<AppState>) -> Vec<DownloadProgress> {
    state
        .downloads
        .lock()
        .unwrap()
        .values()
        .map(|(p, _)| p.clone())
        .collect()
}

#[tauri::command(rename_all = "snake_case")]
fn storage_info(state: State<AppState>) -> StorageInfo {
    let (bytes, count) = downloads::dir_size(&state.store.downloads_dir());
    StorageInfo {
        downloads_bytes: bytes,
        downloads_count: count,
        data_dir: state.store.data_dir.to_string_lossy().to_string(),
    }
}

/// Remet l'application à zéro : abonnements, file d'attente, réglages et téléchargements.
#[tauri::command(rename_all = "snake_case")]
fn reset_app(app: AppHandle, state: State<AppState>) {
    let ids: Vec<String> = state.downloads.lock().unwrap().keys().cloned().collect();
    for id in ids {
        downloads::cancel(&app, &id);
    }
    state.previews.lock().unwrap().clear();
    state.store.update(|l| *l = store::Library::default());
    state.store.flush();
    // Un téléchargement interrompu supprime lui-même son fichier partiel s'il est encore ouvert.
    let _ = std::fs::remove_dir_all(state.store.downloads_dir());
    notify_library(&app);
}

// ─────────────────────────────── Divers ──────────────────────────────

#[tauri::command(rename_all = "snake_case")]
fn open_external(app: AppHandle, url: String) -> Res<()> {
    if !(url.starts_with("http://") || url.starts_with("https://") || url.starts_with("mailto:")) {
        return Err("URL non autorisée".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(err)
}

#[tauri::command(rename_all = "snake_case")]
fn open_data_dir(app: AppHandle, state: State<AppState>) -> Res<()> {
    let dir = state.store.data_dir.to_string_lossy().to_string();
    app.opener().open_path(dir, None::<&str>).map_err(err)
}

#[tauri::command(rename_all = "snake_case")]
async fn import_opml(app: AppHandle) -> Res<ImportReport> {
    let dialog = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        dialog
            .dialog()
            .file()
            .add_filter("OPML", &["opml", "xml"])
            .blocking_pick_file()
    })
    .await
    .map_err(err)?;
    let Some(path) = picked.and_then(|p| p.into_path().ok()) else {
        return Ok(ImportReport::default());
    };
    let bytes = tokio::fs::read(&path).await.map_err(err)?;
    let urls = opml::parse_feed_urls(&opml::decode(&bytes));
    if urls.is_empty() {
        return Err("Aucun flux trouvé dans ce fichier OPML".into());
    }

    let state = app.state::<AppState>();
    let mut report = ImportReport {
        picked: true,
        total: urls.len(),
        ..Default::default()
    };
    let (known, fresh): (Vec<String>, Vec<String>) = urls
        .into_iter()
        .partition(|u| state.store.with(|l| l.podcast(&feed::podcast_id(u)).is_some()));
    report.already = known.len();

    let total = report.total;
    let mut done = report.already;
    let _ = app.emit(EVT_IMPORT, ImportProgress { done, total });
    let mut results = futures_util::stream::iter(fresh)
        .map(|url| {
            let state = app.state::<AppState>();
            async move {
                let ok = subscribe_inner(&state, &url).await.is_ok();
                (url, ok)
            }
        })
        .buffer_unordered(6);
    while let Some((url, ok)) = results.next().await {
        done += 1;
        if ok {
            report.added += 1;
        } else {
            report.failed.push(url);
        }
        let _ = app.emit(EVT_IMPORT, ImportProgress { done, total });
    }
    state.store.flush();
    notify_library(&app);
    Ok(report)
}

#[tauri::command(rename_all = "snake_case")]
async fn export_opml(app: AppHandle) -> Res<bool> {
    let xml = app.state::<AppState>().store.with(|l| {
        let mut podcasts = l.podcasts.clone();
        podcasts.sort_by_key(|p| p.title.to_lowercase());
        opml::export(&podcasts)
    });
    let dialog = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        dialog
            .dialog()
            .file()
            .add_filter("OPML", &["opml"])
            .set_file_name(format!("rustcast-{}.opml", chrono::Local::now().format("%Y-%m-%d")))
            .blocking_save_file()
    })
    .await
    .map_err(err)?;
    let Some(path) = picked.and_then(|p| p.into_path().ok()) else {
        return Ok(false);
    };
    tokio::fs::write(path, xml).await.map_err(err)?;
    Ok(true)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init());
    #[cfg(target_os = "linux")]
    let media = media_server::MediaServer::bind().ok();
    #[cfg(target_os = "linux")]
    let builder = match &media {
        Some(server) => builder.plugin(
            tauri::plugin::Builder::<tauri::Wry>::new("media-server")
                .js_init_script(server.init_script())
                .build(),
        ),
        None => builder,
    };
    builder
        .setup(move |app| {
            let data_dir = app.path().app_data_dir()?;
            let client = reqwest::Client::builder()
                .user_agent(feed::USER_AGENT)
                .timeout(Duration::from_secs(30))
                .build()?;
            let dl_client = reqwest::Client::builder()
                .user_agent(feed::USER_AGENT)
                .connect_timeout(Duration::from_secs(20))
                .read_timeout(Duration::from_secs(60))
                .build()?;
            let legacy_dir = data_dir.with_file_name(store::LEGACY_ID);
            let moved_from = store::migrate_data_dir(&legacy_dir, &data_dir);
            if let Ok(local) = app.path().app_local_data_dir() {
                let legacy_local = local.with_file_name(store::LEGACY_ID);
                if legacy_local != legacy_dir {
                    store::remove_legacy_webview_cache(&legacy_local);
                }
            }
            let store = Arc::new(Store::open(data_dir.clone()));
            if let Some(old) = moved_from {
                store.update(|l| l.rebase_paths(&old, &data_dir));
                store.flush();
            }
            let downloads_dir = store.downloads_dir();
            if store.update(|l| downloads::migrate_names(l, &downloads_dir)) > 0 {
                store.flush();
            }
            let handle = app.handle();
            let _ = app.asset_protocol_scope().allow_directory(&downloads_dir, true);
            let local_files: Vec<std::path::PathBuf> = store.with(|l| {
                l.episodes
                    .values()
                    .flatten()
                    .filter_map(|e| e.local_path.as_ref().map(std::path::PathBuf::from))
                    .collect()
            });
            for file in local_files {
                downloads::allow_playback(handle, &file);
            }
            #[cfg(target_os = "linux")]
            if let Some(server) = media {
                server.spawn(handle.clone());
            }
            app.manage(AppState {
                store: store.clone(),
                client,
                dl_client,
                downloads: Mutex::new(HashMap::new()),
                dl_sem: Arc::new(Semaphore::new(3)),
                previews: Mutex::new(HashMap::new()),
            });

            // Enregistrement différé de la bibliothèque.
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    store.flush();
                }
            });

            // Actualisation des flux au démarrage puis toutes les heures.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_secs(4)).await;
                loop {
                    refresh_all_inner(&handle).await;
                    tokio::time::sleep(Duration::from_secs(3600)).await;
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            list_podcasts,
            get_podcast,
            preview_feed,
            subscribe,
            unsubscribe,
            refresh_podcast,
            refresh_all,
            get_episode,
            get_latest,
            get_in_progress,
            get_downloaded,
            set_progress,
            mark_played,
            mark_all_played,
            get_queue,
            set_queue,
            queue_add,
            queue_remove,
            search_podcasts,
            top_podcasts,
            download_episode,
            cancel_download,
            delete_download,
            reveal_download,
            active_downloads,
            storage_info,
            reset_app,
            open_external,
            open_data_dir,
            import_opml,
            export_opml,
        ])
        .build(tauri::generate_context!())
        .expect("erreur au lancement de RustCast")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.state::<AppState>().store.flush();
            }
        });
}

#[cfg(test)]
mod tests {
    /// Le tag Git, la page « À propos » et l'installateur reposent sur une version unique.
    #[test]
    fn versions_are_in_sync() {
        let version = env!("CARGO_PKG_VERSION");
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(conf["version"], version, "tauri.conf.json");
        for (name, manifest) in [
            ("Cargo.toml (interface)", include_str!("../../Cargo.toml")),
            ("shared/Cargo.toml", include_str!("../../shared/Cargo.toml")),
        ] {
            assert!(manifest.contains(&format!("version = \"{version}\"")), "{name}");
        }
    }
}
