//! Moteur de lecture : un élément vidéo HTML5 unique (il lit aussi l'audio) piloté par des signaux.

use std::cell::Cell;

use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::EpisodeView;
use wasm_bindgen::prelude::*;
use web_sys::HtmlVideoElement;

use crate::api;
use crate::state::{AppState, ToastKind};

#[wasm_bindgen(inline_js = r#"
export function initMediaSession(onPlay, onPause, onBack, onForward, onNext) {
  if (!('mediaSession' in navigator)) return;
  const ms = navigator.mediaSession;
  const set = (a, f) => { try { ms.setActionHandler(a, f); } catch (_) {} };
  set('play', () => onPlay());
  set('pause', () => onPause());
  set('seekbackward', () => onBack());
  set('seekforward', () => onForward());
  set('previoustrack', () => onBack());
  set('nexttrack', () => onNext());
}
export function setMediaMetadata(title, artist, artwork) {
  if (!('mediaSession' in navigator)) return;
  navigator.mediaSession.metadata = new MediaMetadata({
    title, artist, album: artist,
    artwork: artwork ? [{ src: artwork, sizes: '512x512' }] : [],
  });
}
export function clearMediaSession() {
  if (!('mediaSession' in navigator)) return;
  navigator.mediaSession.metadata = null;
  navigator.mediaSession.playbackState = 'none';
}
export function setMediaPlaying(playing) {
  if ('mediaSession' in navigator) navigator.mediaSession.playbackState = playing ? 'playing' : 'paused';
}
export function toggleFullscreen(el) {
  if (document.fullscreenElement) document.exitFullscreen().catch(() => {});
  else el.requestFullscreen().catch(() => {});
}
export function isFullscreen() {
  return !!document.fullscreenElement;
}
export function installFullscreenSync(onChange) {
  // Le plein écran HTML remplit la fenêtre ; on passe aussi la fenêtre Tauri en plein écran.
  document.addEventListener('fullscreenchange', () => {
    const full = !!document.fullscreenElement;
    try { window.__TAURI__.window.getCurrentWindow().setFullscreen(full); } catch (_) {}
    onChange(full);
  });
}
export function installLinkHandler() {
  document.addEventListener('click', (e) => {
    const a = e.target.closest && e.target.closest('a[href]');
    if (!a) return;
    const href = a.getAttribute('href') || '';
    if (/^(https?:|mailto:)/i.test(href)) {
      e.preventDefault();
      window.__TAURI__.core.invoke('open_external', { url: href });
    } else if (!href.startsWith('#')) {
      e.preventDefault();
    }
  }, true);
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = initMediaSession)]
    fn init_media_session(
        on_play: &Closure<dyn Fn()>,
        on_pause: &Closure<dyn Fn()>,
        on_back: &Closure<dyn Fn()>,
        on_forward: &Closure<dyn Fn()>,
        on_next: &Closure<dyn Fn()>,
    );
    #[wasm_bindgen(js_name = setMediaMetadata)]
    fn set_media_metadata(title: &str, artist: &str, artwork: Option<String>);
    #[wasm_bindgen(js_name = clearMediaSession)]
    fn clear_media_session();
    #[wasm_bindgen(js_name = setMediaPlaying)]
    fn set_media_playing(playing: bool);
    #[wasm_bindgen(js_name = installLinkHandler)]
    pub fn install_link_handler();
    #[wasm_bindgen(js_name = toggleFullscreen)]
    fn toggle_fullscreen_js(el: &web_sys::Element);
    #[wasm_bindgen(js_name = isFullscreen)]
    pub fn is_fullscreen() -> bool;
    #[wasm_bindgen(js_name = installFullscreenSync)]
    fn install_fullscreen_sync(on_change: &Closure<dyn Fn(bool)>);
}

thread_local! {
    static MEDIA: HtmlVideoElement = create_media_element();
    /// Conteneur caché qui garde l'élément dans le document quand aucune vue ne l'affiche :
    /// le retirer du document mettrait la lecture en pause.
    static PARKING: web_sys::Element = create_parking();
    static PENDING_SEEK: Cell<Option<f64>> = const { Cell::new(None) };
    static LAST_SAVE: Cell<f64> = const { Cell::new(0.0) };
    /// Vrai quand l'utilisateur a demandé la lecture (évite une erreur au simple préchargement).
    static WANTS_PLAY: Cell<bool> = const { Cell::new(false) };
}

fn create_media_element() -> HtmlVideoElement {
    let el: HtmlVideoElement = document().create_element("video").expect("élément vidéo").unchecked_into();
    let _ = el.set_attribute("playsinline", "");
    let _ = el.set_attribute("class", "media-element");
    el.set_preload("auto");
    PARKING.with(|park| {
        let _ = park.append_child(&el);
    });
    el
}

fn create_parking() -> web_sys::Element {
    let park = document().create_element("div").expect("conteneur");
    let _ = park.set_attribute("id", "media-parking");
    if let Some(body) = document().body() {
        let _ = body.append_child(&park);
    }
    park
}

fn media<R>(f: impl FnOnce(&HtmlVideoElement) -> R) -> R {
    MEDIA.with(f)
}

/// Affiche la vidéo dans `host` (barre de lecture ou vue plein écran).
pub fn attach_video(host: &web_sys::Element) {
    media(|v| {
        let _ = host.append_child(v);
    });
}

/// Rend la vidéo au conteneur caché, si `host` l'affiche encore.
pub fn detach_video(host: &web_sys::Element) {
    media(|v| {
        let is_here = v.parent_element().is_some_and(|p| p.is_same_node(Some(host)));
        if is_here {
            PARKING.with(|park| {
                let _ = park.append_child(v);
            });
        }
    });
}

#[derive(Clone, Copy)]
pub struct Player {
    pub current: RwSignal<Option<EpisodeView>>,
    pub playing: RwSignal<bool>,
    pub buffering: RwSignal<bool>,
    pub time: RwSignal<f64>,
    pub duration: RwSignal<f64>,
    pub rate: RwSignal<f64>,
    pub volume: RwSignal<f64>,
    pub muted: RwSignal<bool>,
    /// Vrai quand le média en cours contient une piste vidéo.
    pub has_video: RwSignal<bool>,
    /// Vrai pendant l'affichage plein écran de la vidéo.
    pub fullscreen: RwSignal<bool>,
    app: AppState,
}

pub fn player() -> Player {
    expect_context::<Player>()
}

fn listen(event: &str, f: impl FnMut() + 'static) {
    let closure = Closure::<dyn FnMut()>::new(f);
    media(|a| {
        let _ = a.add_event_listener_with_callback(event, closure.as_ref().unchecked_ref());
    });
    closure.forget();
}

impl Player {
    pub fn new(app: AppState) -> Self {
        let p = Self {
            current: RwSignal::new(None),
            playing: RwSignal::new(false),
            buffering: RwSignal::new(false),
            time: RwSignal::new(0.0),
            duration: RwSignal::new(0.0),
            rate: RwSignal::new(1.0),
            volume: RwSignal::new(1.0),
            muted: RwSignal::new(false),
            has_video: RwSignal::new(false),
            fullscreen: RwSignal::new(false),
            app,
        };
        media(|a| a.set_preload("auto"));

        listen("timeupdate", move || {
            let t = media(|a| a.current_time());
            p.time.set(t);
            let last = LAST_SAVE.get();
            if (t - last).abs() >= 5.0 {
                LAST_SAVE.set(t);
                p.save_progress();
            }
        });
        listen("loadedmetadata", move || {
            let d = media(|a| a.duration());
            if d.is_finite() {
                p.duration.set(d);
            }
            p.has_video.set(media(|v| v.video_width()) > 0);
            if let Some(seek) = PENDING_SEEK.take() {
                media(|a| a.set_current_time(seek));
            }
        });
        listen("durationchange", move || {
            let d = media(|a| a.duration());
            if d.is_finite() {
                p.duration.set(d);
            }
        });
        listen("play", move || {
            p.playing.set(true);
            set_media_playing(true);
        });
        listen("pause", move || {
            p.playing.set(false);
            set_media_playing(false);
            p.save_progress();
        });
        listen("waiting", move || p.buffering.set(true));
        listen("playing", move || p.buffering.set(false));
        listen("canplay", move || p.buffering.set(false));
        listen("ended", move || p.on_ended());
        listen("error", move || {
            if p.current.get_untracked().is_some() && WANTS_PLAY.get() {
                p.buffering.set(false);
                p.playing.set(false);
                p.app.toast(p.app.tr("toast.play_error"), ToastKind::Error);
            }
        });

        let on_play = Closure::<dyn Fn()>::new(move || p.resume());
        let on_pause = Closure::<dyn Fn()>::new(move || p.pause());
        let on_back = Closure::<dyn Fn()>::new(move || p.skip_back());
        let on_fwd = Closure::<dyn Fn()>::new(move || p.skip_forward());
        let on_next = Closure::<dyn Fn()>::new(move || p.play_next());
        init_media_session(&on_play, &on_pause, &on_back, &on_fwd, &on_next);
        on_play.forget();
        on_pause.forget();
        on_back.forget();
        on_fwd.forget();
        on_next.forget();

        let on_fullscreen = Closure::<dyn Fn(bool)>::new(move |full| p.fullscreen.set(full));
        install_fullscreen_sync(&on_fullscreen);
        on_fullscreen.forget();

        // Applique les réglages de lecture persistés.
        Effect::new(move |_| {
            let (rate, volume) = app.settings.with(|s| (s.playback_rate, s.volume));
            p.rate.set(rate);
            p.volume.set(volume);
            media(|a| {
                a.set_playback_rate(rate);
                a.set_default_playback_rate(rate);
                a.set_volume(volume);
            });
        });
        p
    }

    pub fn restore_last(self, app: AppState) {
        let Some(id) = app.settings.with_untracked(|s| s.last_episode.clone()) else {
            return;
        };
        spawn_local(async move {
            if let Ok(Some(ep)) = api::get_episode(id).await {
                if self.current.get_untracked().is_none() {
                    self.load(ep, false);
                }
            }
        });
    }

    pub fn is_current(self, id: &str) -> bool {
        self.current
            .with(|c| c.as_ref().is_some_and(|e| e.episode.id == id))
    }

    pub fn is_playing(self, id: &str) -> bool {
        self.is_current(id) && self.playing.get()
    }

    fn load(self, ep: EpisodeView, autoplay: bool) {
        self.save_progress();
        let e = &ep.episode;
        let src = match &e.local_path {
            Some(path) => api::file_src(path),
            None => e.audio_url.clone(),
        };
        let start = if e.played {
            0.0
        } else {
            match e.duration {
                Some(d) if e.position > d as f64 - 10.0 => 0.0,
                _ => e.position,
            }
        };
        PENDING_SEEK.set(Some(start).filter(|s| *s > 1.0));
        LAST_SAVE.set(start);
        self.time.set(start);
        self.duration.set(e.duration.unwrap_or(0) as f64);
        set_media_metadata(&e.title, &ep.podcast_title, ep.artwork());
        // Première estimation d'après le flux, confirmée au chargement des métadonnées.
        self.has_video.set(e.is_video());
        if is_fullscreen() && !e.is_video() {
            self.toggle_fullscreen();
        }
        WANTS_PLAY.set(false);
        media(|a| {
            a.set_src(&src);
            a.set_playback_rate(self.rate.get_untracked());
            a.set_default_playback_rate(self.rate.get_untracked());
        });
        self.current.set(Some(ep));
        if autoplay {
            self.buffering.set(true);
            self.resume();
        }
    }

    pub fn play(self, ep: EpisodeView) {
        let id = ep.episode.id.clone();
        if self.is_current(&id) {
            self.resume();
            return;
        }
        // Lire un épisode de la file d'attente l'en retire.
        if self.app.is_queued(&id) {
            let app = self.app;
            let qid = id.clone();
            spawn_local(async move {
                let _ = api::queue_remove(qid).await;
                app.reload_library().await;
            });
        }
        self.load(ep, true);
        self.app.update_settings(|s| s.last_episode = Some(id));
    }

    pub fn toggle_episode(self, ep: EpisodeView) {
        if self.is_current(&ep.episode.id) {
            self.toggle();
        } else {
            self.play(ep);
        }
    }

    pub fn resume(self) {
        if self.current.get_untracked().is_none() {
            return;
        }
        WANTS_PLAY.set(true);
        // Un échec au préchargement (au démarrage par exemple) bloque play() :
        // on recharge la source pour retenter réellement la lecture.
        if media(|a| a.error().is_some()) {
            self.buffering.set(true);
            media(|a| a.load());
        }
        let app = self.app;
        media(|a| {
            if let Ok(promise) = a.play() {
                spawn_local(async move {
                    if let Err(e) = wasm_bindgen_futures::JsFuture::from(promise).await {
                        let name = js_sys::Reflect::get(&e, &JsValue::from_str("name"))
                            .ok()
                            .and_then(|n| n.as_string())
                            .unwrap_or_default();
                        // AbortError : lecture interrompue par un changement d'épisode.
                        // NotSupportedError : déjà signalé par l'événement « error ».
                        if name != "AbortError" && name != "NotSupportedError" {
                            self.buffering.set(false);
                            app.toast(app.tr("toast.play_error"), ToastKind::Error);
                        }
                    }
                });
            }
        });
    }

    pub fn pause(self) {
        media(|a| {
            let _ = a.pause();
        });
    }

    pub fn toggle(self) {
        if self.playing.get_untracked() {
            self.pause();
        } else {
            self.resume();
        }
    }

    /// Ferme l'épisode en cours : enregistre la position, arrête la lecture et masque le lecteur.
    /// L'épisode reste disponible dans « Reprendre l'écoute ».
    pub fn close(self) {
        let Some(ep) = self.current.get_untracked() else {
            return;
        };
        let (t, d) = media(|a| (a.current_time(), a.duration()));
        let duration = d.is_finite().then_some(d);
        let position = if PENDING_SEEK.get().is_some() { ep.episode.position } else { t };
        WANTS_PLAY.set(false);
        PENDING_SEEK.set(None);
        if is_fullscreen() {
            self.toggle_fullscreen();
        }
        media(|a| {
            let _ = a.pause();
            let _ = a.remove_attribute("src");
            a.load();
        });
        clear_media_session();
        self.current.set(None);
        self.playing.set(false);
        self.buffering.set(false);
        self.has_video.set(false);
        self.time.set(0.0);
        self.duration.set(0.0);
        self.app.now_playing_open.set(false);
        self.app.update_settings(|s| s.last_episode = None);
        let app = self.app;
        spawn_local(async move {
            let _ = api::set_progress(ep.episode.id, position, duration).await;
            // Met à jour « Reprendre l'écoute » avec la position enregistrée.
            app.reload_library().await;
        });
    }

    pub fn toggle_fullscreen(self) {
        media(|v| toggle_fullscreen_js(v.as_ref()));
    }

    pub fn seek(self, t: f64) {
        let d = self.duration.get_untracked();
        let t = if d > 0.0 { t.clamp(0.0, d) } else { t.max(0.0) };
        media(|a| a.set_current_time(t));
        self.time.set(t);
    }

    pub fn skip(self, delta: f64) {
        self.seek(media(|a| a.current_time()) + delta);
    }

    pub fn skip_back(self) {
        self.skip(-(self.app.settings.with_untracked(|s| s.skip_back) as f64));
    }

    pub fn skip_forward(self) {
        self.skip(self.app.settings.with_untracked(|s| s.skip_forward) as f64);
    }

    pub fn set_rate(self, rate: f64) {
        self.app.update_settings(|s| s.playback_rate = rate);
    }

    pub fn set_volume(self, v: f64) {
        self.muted.set(false);
        media(|a| a.set_muted(false));
        self.app.update_settings(|s| s.volume = v.clamp(0.0, 1.0));
    }

    pub fn toggle_mute(self) {
        let m = !self.muted.get_untracked();
        self.muted.set(m);
        media(|a| a.set_muted(m));
    }

    fn save_progress(self) {
        let Some(ep) = self.current.get_untracked() else {
            return;
        };
        let (t, d) = media(|a| (a.current_time(), a.duration()));
        if t <= 0.0 && PENDING_SEEK.get().is_some() {
            return;
        }
        let duration = d.is_finite().then_some(d);
        spawn_local(async move {
            let _ = api::set_progress(ep.episode.id, t, duration).await;
        });
    }

    fn on_ended(self) {
        let Some(ep) = self.current.get_untracked() else {
            return;
        };
        let app = self.app;
        spawn_local(async move {
            let _ = api::mark_played(ep.episode.id.clone(), true).await;
            app.reload_library().await;
            self.play_next();
        });
    }

    pub fn play_next(self) {
        let current_id = self.current.with_untracked(|c| c.as_ref().map(|e| e.episode.id.clone()));
        let next = self.app.queue.with_untracked(|q| {
            q.iter()
                .find(|e| Some(&e.episode.id) != current_id.as_ref())
                .cloned()
        });
        if let Some(ep) = next {
            self.play(ep);
        }
    }
}
