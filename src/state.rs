//! État global de l'interface, partagé par contexte.

use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::*;

use crate::api;
use crate::i18n;

#[derive(Clone, Debug, PartialEq)]
pub enum View {
    Home,
    Library,
    Discover,
    Downloads,
    Queue,
    Settings,
    Podcast(String),
    Preview(String),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    pub id: u32,
    pub msg: String,
    pub kind: ToastKind,
}

#[derive(Clone, Copy)]
pub struct AppState {
    pub settings: RwSignal<Settings>,
    pub view: RwSignal<View>,
    pub history: RwSignal<Vec<View>>,
    pub podcasts: RwSignal<Vec<PodcastSummary>>,
    pub downloads: RwSignal<HashMap<String, DownloadProgress>>,
    pub queue: RwSignal<Vec<EpisodeView>>,
    pub toasts: RwSignal<Vec<Toast>>,
    /// Incrémenté à chaque modification de la bibliothèque côté backend.
    pub library_rev: RwSignal<u32>,
    pub modal_episode: RwSignal<Option<EpisodeView>>,
    pub now_playing_open: RwSignal<bool>,
    pub refreshing: RwSignal<bool>,
    pub ready: RwSignal<bool>,
    /// Avancement de l'import OPML en cours.
    pub importing: RwSignal<Option<ImportProgress>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            settings: RwSignal::new(Settings::default()),
            view: RwSignal::new(View::Home),
            history: RwSignal::new(vec![]),
            podcasts: RwSignal::new(vec![]),
            downloads: RwSignal::new(HashMap::new()),
            queue: RwSignal::new(vec![]),
            toasts: RwSignal::new(vec![]),
            library_rev: RwSignal::new(0),
            modal_episode: RwSignal::new(None),
            now_playing_open: RwSignal::new(false),
            refreshing: RwSignal::new(false),
            ready: RwSignal::new(false),
            importing: RwSignal::new(None),
        }
    }

    pub fn init(self, player: crate::player::Player) {
        spawn_local(async move {
            if let Ok(s) = api::get_settings().await {
                self.settings.set(s);
            }
            self.reload_library().await;
            if let Ok(list) = api::active_downloads().await {
                self.downloads
                    .set(list.into_iter().map(|d| (d.episode_id.clone(), d)).collect());
            }
            self.ready.set(true);
            player.restore_last(self);
        });

        api::listen::<()>(EVT_LIBRARY, move |_| {
            spawn_local(async move { self.reload_library().await });
        });
        api::listen::<ImportProgress>(EVT_IMPORT, move |p| {
            if self.importing.get_untracked().is_some() {
                self.importing.set(Some(p));
            }
        });
        api::listen::<DownloadProgress>(EVT_DOWNLOAD, move |p| {
            let done = matches!(p.status, DownloadStatus::Done);
            let title = p.title.clone();
            self.downloads.update(|m| match p.status {
                DownloadStatus::Cancelled | DownloadStatus::Done => {
                    m.remove(&p.episode_id);
                }
                _ => {
                    m.insert(p.episode_id.clone(), p);
                }
            });
            if done {
                self.toast(format!("{} · {}", self.tr("toast.downloaded"), title), ToastKind::Success);
            }
        });
    }

    pub async fn reload_library(self) {
        if let Ok(list) = api::list_podcasts().await {
            self.podcasts.set(list);
        }
        if let Ok(q) = api::get_queue().await {
            self.queue.set(q);
        }
        self.library_rev.update(|r| *r += 1);
    }

    pub fn lang(self) -> Lang {
        self.settings.with(|s| s.language)
    }

    /// Traduction réactive d'une clé.
    pub fn tr(self, key: &'static str) -> &'static str {
        i18n::tr(self.lang(), key)
    }

    pub fn navigate(self, view: View) {
        let current = self.view.get_untracked();
        if current == view {
            return;
        }
        self.history.update(|h| {
            h.push(current);
            if h.len() > 50 {
                h.remove(0);
            }
        });
        self.view.set(view);
        self.now_playing_open.set(false);
        scroll_main_top();
    }

    /// Navigation de premier niveau (barre latérale) : réinitialise l'historique.
    pub fn go_root(self, view: View) {
        self.history.set(vec![]);
        self.view.set(view);
        self.now_playing_open.set(false);
        scroll_main_top();
    }

    pub fn back(self) {
        let prev = self.history.try_update(|h| h.pop()).flatten();
        if let Some(v) = prev {
            self.view.set(v);
            scroll_main_top();
        }
    }

    pub fn toast(self, msg: impl Into<String>, kind: ToastKind) {
        let id = (js_sys::Math::random() * 1e9) as u32;
        self.toasts.update(|t| t.push(Toast { id, msg: msg.into(), kind }));
        set_timeout(
            move || self.toasts.update(|t| t.retain(|x| x.id != id)),
            Duration::from_millis(3800),
        );
    }

    pub fn error(self, e: String) {
        self.toast(format!("{} — {e}", self.tr("toast.error")), ToastKind::Error);
    }

    pub fn update_settings(self, f: impl FnOnce(&mut Settings)) {
        self.settings.update(f);
        let s = self.settings.get_untracked();
        spawn_local(async move {
            let _ = api::save_settings(s).await;
        });
    }

    pub fn is_subscribed(self, feed_url: &str) -> bool {
        self.podcasts
            .with(|l| l.iter().any(|p| p.podcast.feed_url == feed_url))
    }

    pub fn refresh_all(self) {
        if self.refreshing.get_untracked() {
            return;
        }
        self.refreshing.set(true);
        spawn_local(async move {
            match api::refresh_all().await {
                Ok(0) => self.toast(self.tr("toast.up_to_date"), ToastKind::Info),
                Ok(n) => self.toast(format!("{n} {}", self.tr("toast.new_episodes")), ToastKind::Success),
                Err(e) => self.error(e),
            }
            self.refreshing.set(false);
        });
    }

    pub fn subscribe(self, feed_url: String) {
        spawn_local(async move {
            match api::subscribe(feed_url).await {
                Ok(s) => self.toast(
                    format!("{} « {} »", self.tr("toast.subscribed"), s.podcast.title),
                    ToastKind::Success,
                ),
                Err(e) => self.error(e),
            }
        });
    }

    pub fn download(self, id: String) {
        spawn_local(async move {
            if let Err(e) = api::download_episode(id).await {
                self.error(e);
            }
        });
    }

    pub fn queue_add(self, ep: &EpisodeView, next: bool) {
        let id = ep.episode.id.clone();
        spawn_local(async move {
            if api::queue_add(id, next).await.is_ok() {
                let key = if next { "toast.play_next" } else { "toast.queued" };
                self.toast(self.tr(key), ToastKind::Info);
            }
        });
    }

    pub fn import_opml(self) {
        if self.importing.get_untracked().is_some() {
            return;
        }
        self.importing.set(Some(ImportProgress { done: 0, total: 0 }));
        spawn_local(async move {
            let result = api::import_opml().await;
            self.importing.set(None);
            match result {
                Ok(r) if !r.picked => {}
                Ok(r) => {
                    let mut parts = vec![format!("{} {}", r.added, self.tr("opml.added"))];
                    if r.already > 0 {
                        parts.push(format!("{} {}", r.already, self.tr("opml.already")));
                    }
                    if !r.failed.is_empty() {
                        parts.push(format!("{} {}", r.failed.len(), self.tr("opml.failed")));
                    }
                    let kind = if r.added == 0 && !r.failed.is_empty() {
                        ToastKind::Error
                    } else if r.added > 0 {
                        ToastKind::Success
                    } else {
                        ToastKind::Info
                    };
                    self.toast(format!("{} — {}", self.tr("opml.imported"), parts.join(" · ")), kind);
                    if r.added > 0 && matches!(self.view.get_untracked(), View::Home | View::Settings) {
                        self.go_root(View::Library);
                    }
                }
                Err(e) => self.error(e),
            }
        });
    }

    pub fn export_opml(self) {
        if self.podcasts.with_untracked(Vec::is_empty) {
            self.toast(self.tr("opml.nothing"), ToastKind::Info);
            return;
        }
        spawn_local(async move {
            match api::export_opml().await {
                Ok(true) => self.toast(self.tr("toast.exported"), ToastKind::Success),
                Ok(false) => {}
                Err(e) => self.error(e),
            }
        });
    }

    pub fn is_queued(self, id: &str) -> bool {
        self.queue.with(|q| q.iter().any(|e| e.episode.id == id))
    }
}

pub fn app() -> AppState {
    expect_context::<AppState>()
}

fn scroll_main_top() {
    if let Some(el) = document().get_element_by_id("main-scroll") {
        el.set_scroll_top(0);
    }
}

/// Charge une donnée de façon asynchrone et la recharge quand ses dépendances changent.
pub fn async_data<T, Fut>(fetch: impl Fn() -> Fut + 'static) -> RwSignal<Option<Result<T, String>>>
where
    T: Send + Sync + 'static,
    Fut: Future<Output = Result<T, String>> + 'static,
{
    let data = RwSignal::new(None);
    let generation = StoredValue::new(0u32);
    Effect::new(move |_| {
        let fut = fetch();
        generation.update_value(|g| *g += 1);
        let current = generation.get_value();
        spawn_local(async move {
            let result = fut.await;
            // Ignore les réponses arrivées après une requête plus récente.
            if generation.try_get_value() == Some(current) {
                data.try_set(Some(result));
            }
        });
    });
    data
}
