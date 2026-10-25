//! Pont vers les commandes Tauri exposées par le backend.

use serde::de::DeserializeOwned;
use serde::Serialize;
use shared::*;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke, catch)]
    async fn tauri_invoke(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = convertFileSrc)]
    fn tauri_convert_file_src(path: &str) -> String;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"], js_name = listen)]
    async fn tauri_listen(event: &str, handler: &Closure<dyn FnMut(JsValue)>) -> JsValue;
}

fn to_js<A: Serialize>(args: &A) -> JsValue {
    args.serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .unwrap_or(JsValue::UNDEFINED)
}

pub async fn call<R: DeserializeOwned>(cmd: &str, args: JsValue) -> Result<R, String> {
    match tauri_invoke(cmd, args).await {
        Ok(v) => serde_wasm_bindgen::from_value(v).map_err(|e| e.to_string()),
        Err(e) => Err(e.as_string().unwrap_or_else(|| format!("{e:?}"))),
    }
}

macro_rules! args {
    ($($k:ident : $v:expr),* $(,)?) => {{
        let obj = js_sys::Object::new();
        $( let _ = js_sys::Reflect::set(&obj, &JsValue::from_str(stringify!($k)), &to_js(&$v)); )*
        JsValue::from(obj)
    }};
}

pub fn file_src(path: &str) -> String {
    tauri_convert_file_src(path)
}

/// Écoute un événement Tauri ; la fermeture reste active pendant toute la vie de l'application.
pub fn listen<T: DeserializeOwned + 'static>(event: &'static str, mut f: impl FnMut(T) + 'static) {
    let closure = Closure::<dyn FnMut(JsValue)>::new(move |evt: JsValue| {
        let payload = js_sys::Reflect::get(&evt, &JsValue::from_str("payload")).unwrap_or(JsValue::NULL);
        if let Ok(v) = serde_wasm_bindgen::from_value::<T>(payload) {
            f(v);
        }
    });
    wasm_bindgen_futures::spawn_local(async move {
        tauri_listen(event, &closure).await;
        closure.forget();
    });
}

// ───────────────────────────── Commandes ─────────────────────────────

pub async fn get_settings() -> Result<Settings, String> {
    call("get_settings", args!()).await
}
pub async fn save_settings(settings: Settings) -> Result<(), String> {
    call("save_settings", args!(settings: settings)).await
}
pub async fn list_podcasts() -> Result<Vec<PodcastSummary>, String> {
    call("list_podcasts", args!()).await
}
pub async fn get_podcast(id: String) -> Result<FeedPreview, String> {
    call("get_podcast", args!(id: id)).await
}
pub async fn preview_feed(feed_url: String) -> Result<FeedPreview, String> {
    call("preview_feed", args!(feed_url: feed_url)).await
}
pub async fn subscribe(feed_url: String) -> Result<PodcastSummary, String> {
    call("subscribe", args!(feed_url: feed_url)).await
}
pub async fn unsubscribe(id: String) -> Result<(), String> {
    call("unsubscribe", args!(id: id)).await
}
pub async fn refresh_podcast(id: String) -> Result<usize, String> {
    call("refresh_podcast", args!(id: id)).await
}
pub async fn refresh_all() -> Result<usize, String> {
    call("refresh_all", args!()).await
}
pub async fn get_episode(id: String) -> Result<Option<EpisodeView>, String> {
    call("get_episode", args!(id: id)).await
}
pub async fn get_latest(limit: usize) -> Result<Vec<EpisodeView>, String> {
    call("get_latest", args!(limit: limit)).await
}
pub async fn get_in_progress() -> Result<Vec<EpisodeView>, String> {
    call("get_in_progress", args!()).await
}
pub async fn get_downloaded() -> Result<Vec<EpisodeView>, String> {
    call("get_downloaded", args!()).await
}
pub async fn set_progress(id: String, position: f64, duration: Option<f64>) -> Result<(), String> {
    call("set_progress", args!(id: id, position: position, duration: duration)).await
}
pub async fn mark_played(id: String, played: bool) -> Result<(), String> {
    call("mark_played", args!(id: id, played: played)).await
}
pub async fn mark_all_played(podcast_id: String) -> Result<(), String> {
    call("mark_all_played", args!(podcast_id: podcast_id)).await
}
pub async fn get_queue() -> Result<Vec<EpisodeView>, String> {
    call("get_queue", args!()).await
}
pub async fn set_queue(ids: Vec<String>) -> Result<(), String> {
    call("set_queue", args!(ids: ids)).await
}
pub async fn queue_add(id: String, next: bool) -> Result<(), String> {
    call("queue_add", args!(id: id, next: next)).await
}
pub async fn queue_remove(id: String) -> Result<(), String> {
    call("queue_remove", args!(id: id)).await
}
pub async fn search_podcasts(term: String, country: String) -> Result<Vec<SearchResult>, String> {
    call("search_podcasts", args!(term: term, country: country)).await
}
pub async fn top_podcasts(country: String) -> Result<Vec<SearchResult>, String> {
    call("top_podcasts", args!(country: country)).await
}
pub async fn download_episode(id: String) -> Result<(), String> {
    call("download_episode", args!(id: id)).await
}
pub async fn cancel_download(id: String) -> Result<(), String> {
    call("cancel_download", args!(id: id)).await
}
pub async fn delete_download(id: String) -> Result<(), String> {
    call("delete_download", args!(id: id)).await
}
pub async fn reveal_download(id: String) -> Result<(), String> {
    call("reveal_download", args!(id: id)).await
}
pub async fn active_downloads() -> Result<Vec<DownloadProgress>, String> {
    call("active_downloads", args!()).await
}
pub async fn storage_info() -> Result<StorageInfo, String> {
    call("storage_info", args!()).await
}
pub async fn open_data_dir() -> Result<(), String> {
    call("open_data_dir", args!()).await
}
pub async fn import_opml() -> Result<ImportReport, String> {
    call("import_opml", args!()).await
}
pub async fn export_opml() -> Result<bool, String> {
    call("export_opml", args!()).await
}
