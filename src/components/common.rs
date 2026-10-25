//! Petits composants réutilisables : pochette, boutons, état vide.

use leptos::prelude::*;
use shared::{DownloadStatus, EpisodeView};

use crate::icons::Icon;
use crate::player::player;
use crate::state::app;
use crate::util::{fmt_duration, fmt_remaining};

/// Pochette avec dégradé et initiale de repli si l'image est absente ou en erreur.
#[component]
pub fn Artwork(
    #[prop(into)] src: Signal<Option<String>>,
    #[prop(into)] title: Signal<String>,
    #[prop(optional, into)] class: String,
) -> impl IntoView {
    let failed = RwSignal::new(false);
    Effect::new(move |_| {
        src.track();
        failed.set(false);
    });
    let hue = move || {
        title.with(|t| t.bytes().fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32)) % 360)
    };
    let initial = move || {
        title.with(|t| t.chars().find(|c| c.is_alphanumeric()).unwrap_or('♪').to_uppercase().to_string())
    };
    view! {
        <div class=format!("art {class}")>
            {move || match src.get().filter(|_| !failed.get()) {
                Some(url) => view! {
                    <img src=url alt="" loading="lazy" draggable="false" on:error=move |_| failed.set(true) />
                }
                .into_any(),
                None => view! {
                    <div
                        class="art-fallback"
                        style=move || format!(
                            "background: linear-gradient(135deg, hsl({} 70% 55%), hsl({} 75% 40%))",
                            hue(),
                            (hue() + 50) % 360,
                        )
                    >
                        <span>{initial}</span>
                    </div>
                }
                .into_any(),
            }}
        </div>
    }
}

/// Barres animées signalant l'épisode en cours de lecture.
#[component]
pub fn Equalizer(#[prop(into)] active: Signal<bool>) -> impl IntoView {
    view! {
        <span class="eq" class:paused=move || !active.get()>
            <i></i>
            <i></i>
            <i></i>
        </span>
    }
}

/// Pastille de lecture affichant la durée ou le temps restant et la progression.
#[component]
pub fn PlayPill(ep: EpisodeView) -> impl IntoView {
    let app = app();
    let player = player();
    let id = ep.episode.id.clone();
    let duration = ep.episode.duration;
    let saved = ep.episode.position;
    let played = ep.episode.played;
    let is_current = Memo::new(move |_| player.is_current(&id));
    let playing = Signal::derive(move || is_current.get() && player.playing.get());

    let position = move || if is_current.get() { player.time.get() } else { saved };
    let total = move || {
        if is_current.get() && player.duration.get() > 0.0 {
            player.duration.get()
        } else {
            duration.unwrap_or(0) as f64
        }
    };
    let ratio = move || {
        let t = total();
        if t > 0.0 { (position() / t).clamp(0.0, 1.0) } else { 0.0 }
    };
    let label = move || {
        let lang = app.lang();
        if is_current.get() && player.buffering.get() {
            return app.tr("player.loading").to_string();
        }
        if played && !is_current.get() {
            return app.tr("ep.played").to_string();
        }
        let t = total();
        let p = position();
        if p > 5.0 && t > 0.0 {
            fmt_remaining(lang, t - p)
        } else if t > 0.0 {
            fmt_duration(t as u32)
        } else {
            app.tr("ep.play").to_string()
        }
    };
    let ep_click = ep.clone();
    view! {
        <button
            class="play-pill"
            class:active=move || is_current.get()
            class:played=move || played && !is_current.get()
            title=move || app.tr(if playing.get() { "player.pause" } else { "player.play" })
            on:click=move |ev| {
                ev.stop_propagation();
                player.toggle_episode(ep_click.clone());
            }
        >
            <span class="pill-icon">
                {move || if playing.get() {
                    view! { <Equalizer active=playing /> }.into_any()
                } else {
                    view! { <Icon name="play" size=12 /> }.into_any()
                }}
            </span>
            <span class="pill-label">{label}</span>
            <Show when=move || { ratio() > 0.0 && !(played && !is_current.get()) }>
                <span class="pill-progress">
                    <span style:width=move || format!("{:.1}%", ratio() * 100.0)></span>
                </span>
            </Show>
        </button>
    }
}

/// Bouton de téléchargement avec anneau de progression.
#[component]
pub fn DownloadButton(ep: EpisodeView) -> impl IntoView {
    let app = app();
    let id = ep.episode.id.clone();
    let downloaded = ep.episode.local_path.is_some();
    let progress = {
        let id = id.clone();
        Memo::new(move |_| app.downloads.with(|m| m.get(&id).cloned()))
    };
    let circumference = 2.0 * std::f64::consts::PI * 9.0;
    view! {
        {move || {
            let id = id.clone();
            match progress.get() {
                Some(p) if matches!(p.status, DownloadStatus::Queued | DownloadStatus::Downloading) => {
                    let ratio = p.ratio();
                    let indeterminate = p.total.is_none() || matches!(p.status, DownloadStatus::Queued);
                    view! {
                        <button
                            class="icon-btn dl-btn downloading"
                            class:indeterminate=indeterminate
                            title=app.tr("dl.cancel")
                            on:click=move |ev| {
                                ev.stop_propagation();
                                let id = id.clone();
                                leptos::task::spawn_local(async move {
                                    let _ = crate::api::cancel_download(id).await;
                                });
                            }
                        >
                            <svg class="ring" width="26" height="26" viewBox="0 0 24 24">
                                <circle class="ring-bg" cx="12" cy="12" r="9" />
                                <circle
                                    class="ring-fg"
                                    cx="12"
                                    cy="12"
                                    r="9"
                                    stroke-dasharray=format!("{circumference:.2}")
                                    stroke-dashoffset=format!(
                                        "{:.2}",
                                        if indeterminate { circumference * 0.7 } else { circumference * (1.0 - ratio) },
                                    )
                                />
                            </svg>
                            <span class="ring-stop"></span>
                        </button>
                    }
                    .into_any()
                }
                Some(p) if matches!(p.status, DownloadStatus::Failed(_)) => {
                    let msg = match &p.status {
                        DownloadStatus::Failed(m) => m.clone(),
                        _ => String::new(),
                    };
                    view! {
                        <button
                            class="icon-btn dl-btn failed"
                            title=format!("{} — {msg}", app.tr("dl.retry"))
                            on:click=move |ev| {
                                ev.stop_propagation();
                                app.download(id.clone());
                            }
                        >
                            <Icon name="refresh" size=18 />
                        </button>
                    }
                    .into_any()
                }
                _ if downloaded => view! {
                    <span class="icon-btn dl-btn done" title=app.tr("dl.downloaded")>
                        <Icon name="check" size=18 />
                    </span>
                }
                .into_any(),
                _ => view! {
                    <button
                        class="icon-btn dl-btn"
                        title=app.tr("dl.download")
                        on:click=move |ev| {
                            ev.stop_propagation();
                            app.download(id.clone());
                        }
                    >
                        <Icon name="download" size=18 />
                    </button>
                }
                .into_any(),
            }
        }}
    }
}

#[component]
pub fn EmptyState(
    icon: &'static str,
    #[prop(into)] title: Signal<String>,
    #[prop(into)] text: Signal<String>,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    view! {
        <div class="empty">
            <div class="empty-icon">
                <Icon name=icon size=34 />
            </div>
            <h3>{title}</h3>
            <p>{text}</p>
            {children.map(|c| c())}
        </div>
    }
}

#[component]
pub fn Spinner() -> impl IntoView {
    view! { <div class="spinner"></div> }
}

#[component]
pub fn PageHeader(
    #[prop(into)] title: Signal<String>,
    #[prop(optional, into)] subtitle: Option<Signal<String>>,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    view! {
        <header class="page-header">
            <div>
                <h1>{title}</h1>
                {subtitle.map(|s| view! { <p class="subtitle">{s}</p> })}
            </div>
            <div class="header-actions">{children.map(|c| c())}</div>
        </header>
    }
}
