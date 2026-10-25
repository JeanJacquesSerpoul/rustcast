//! Ligne d'épisode, menu contextuel et fenêtre de notes d'épisode.

use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::EpisodeView;

use crate::api;
use crate::components::common::{Artwork, DownloadButton, PlayPill};
use crate::icons::Icon;
use crate::player::player;
use crate::state::{app, ToastKind, View};
use crate::util::{fmt_bytes_lang, fmt_date, fmt_duration};

#[component]
pub fn EpisodeRow(ep: EpisodeView, #[prop(default = false)] show_podcast: bool) -> impl IntoView {
    let app = app();
    let player = player();
    let id = ep.episode.id.clone();
    let is_current = Memo::new(move |_| player.is_current(&id));
    let e = &ep.episode;
    let played = e.played;

    let meta = {
        let (date, dur, season, number) = (e.pub_date, e.duration, e.season, e.number);
        move || {
            let lang = app.lang();
            let mut parts = vec![fmt_date(lang, date)];
            if let Some(n) = number {
                parts.push(match season {
                    Some(s) => format!("S{s} · É{n}").replace('É', if lang == shared::Lang::En { "E" } else { "É" }),
                    None => format!("#{n}"),
                });
            }
            if let Some(d) = dur {
                parts.push(fmt_duration(d));
            }
            parts.retain(|p| !p.is_empty());
            parts.join(" · ")
        }
    };

    let open_detail = {
        let ep = ep.clone();
        move |_| app.modal_episode.set(Some(ep.clone()))
    };
    let open_podcast = {
        let pid = ep.episode.podcast_id.clone();
        move |ev: leptos::ev::MouseEvent| {
            ev.stop_propagation();
            app.navigate(View::Podcast(pid.clone()));
        }
    };

    view! {
        <article class="ep-row" class:current=move || is_current.get() class:is-played=played on:click=open_detail>
            <Artwork
                src=ep.artwork()
                title=if show_podcast { ep.podcast_title.clone() } else { ep.episode.title.clone() }
                class="ep-art"
            />
            <div class="ep-body">
                <div class="ep-meta">
                    {show_podcast.then(|| view! {
                        <button class="ep-podcast link" on:click=open_podcast.clone()>{ep.podcast_title.clone()}</button>
                        <span class="dot">"•"</span>
                    })}
                    <span>{meta}</span>
                    {ep.episode.is_video().then(|| view! {
                        <span class="ep-badge">
                            <Icon name="video" size=12 />
                            {move || app.tr("ep.video")}
                        </span>
                    })}
                </div>
                <h3 class="ep-title">{ep.episode.title.clone()}</h3>
                <p class="ep-summary">{ep.episode.summary.clone()}</p>
                <div class="ep-actions" on:click=|ev| ev.stop_propagation()>
                    <PlayPill ep=ep.clone() />
                    <div class="spacer"></div>
                    <DownloadButton ep=ep.clone() />
                    <EpisodeMenu ep=ep.clone() />
                </div>
            </div>
        </article>
    }
}

#[component]
pub fn EpisodeMenu(ep: EpisodeView, #[prop(default = false)] up: bool) -> impl IntoView {
    let app = app();
    let open = RwSignal::new(false);
    let flip = RwSignal::new(up);
    let wrap = NodeRef::<leptos::html::Div>::new();
    let ep = StoredValue::new(ep);
    let close = move || open.set(false);

    let item = move |icon: &'static str, key: &'static str, action: Box<dyn Fn() + Send + Sync>| {
        view! {
            <button
                class="menu-item"
                on:click=move |ev| {
                    ev.stop_propagation();
                    action();
                    close();
                }
            >
                <Icon name=icon size=17 />
                <span>{move || app.tr(key)}</span>
            </button>
        }
    };

    view! {
        <div class="menu-wrap" node_ref=wrap on:click=|ev| ev.stop_propagation()>
            <button
                class="icon-btn"
                class:active=move || open.get()
                title=move || app.tr("menu.more")
                on:click=move |_| {
                    // Ouvre le menu vers le haut s'il manque de place sous le bouton.
                    if let Some(el) = wrap.get_untracked() {
                        let bottom = el.get_bounding_client_rect().bottom();
                        let height = window().inner_height().ok().and_then(|h| h.as_f64()).unwrap_or(800.0);
                        flip.set(up || height - bottom < 380.0);
                    }
                    open.update(|o| *o = !*o)
                }
            >
                <Icon name="more" size=18 />
            </button>
            <Show when=move || open.get()>
                <div class="menu-backdrop" on:click=move |_| close()></div>
                <div class="menu" class:up=move || flip.get()>
                    {item("play-next", "menu.play_next", Box::new(move || app.queue_add(&ep.get_value(), true)))}
                    {item("list-plus", "menu.queue", Box::new(move || app.queue_add(&ep.get_value(), false)))}
                    {move || {
                        let e = ep.get_value();
                        let played = e.episode.played;
                        let id = e.episode.id.clone();
                        item(
                            if played { "circle" } else { "check-circle" },
                            if played { "menu.mark_unplayed" } else { "menu.mark_played" },
                            Box::new(move || {
                                let id = id.clone();
                                spawn_local(async move {
                                    let _ = api::mark_played(id, !played).await;
                                });
                            }),
                        )
                    }}
                    {move || {
                        let e = ep.get_value();
                        e.episode.local_path.is_some().then(|| {
                            let id = e.episode.id.clone();
                            item(
                                "trash",
                                "menu.delete_download",
                                Box::new(move || {
                                    let id = id.clone();
                                    spawn_local(async move {
                                        if api::delete_download(id).await.is_ok() {
                                            app.toast(app.tr("toast.deleted"), ToastKind::Info);
                                        }
                                    });
                                }),
                            )
                        })
                    }}
                    {move || {
                        let e = ep.get_value();
                        e.episode.local_path.is_some().then(|| {
                            let id = e.episode.id.clone();
                            item(
                                "folder",
                                "menu.reveal",
                                Box::new(move || {
                                    let id = id.clone();
                                    spawn_local(async move {
                                        if let Err(e) = api::reveal_download(id).await {
                                            app.error(e);
                                        }
                                    });
                                }),
                            )
                        })
                    }}
                    <div class="menu-sep"></div>
                    {item("notes", "menu.notes", Box::new(move || app.modal_episode.set(Some(ep.get_value()))))}
                    {item(
                        "library",
                        "menu.go_podcast",
                        Box::new(move || app.navigate(View::Podcast(ep.get_value().episode.podcast_id))),
                    )}
                </div>
            </Show>
        </div>
    }
}

/// Fenêtre modale des notes d'épisode.
#[component]
pub fn EpisodeModal() -> impl IntoView {
    let app = app();
    let player = player();
    let close = move || app.modal_episode.set(None);

    view! {
        <Show when=move || app.modal_episode.with(Option::is_some)>
            {move || {
                app.modal_episode.get().map(|ep| {
                    let e = ep.episode.clone();
                    let id = e.id.clone();
                    let playing = Signal::derive(move || player.is_playing(&id));
                    let ep_play = ep.clone();
                    let pid = e.podcast_id.clone();
                    let size = e.local_size.or(e.size);
                    view! {
                        <div class="modal-backdrop" on:click=move |_| close()>
                            <div class="modal" on:click=|ev| ev.stop_propagation()>
                                <div class="modal-hero">
                                    <div
                                        class="modal-bg"
                                        style=ep.artwork().map(|a| format!("background-image:url('{a}')")).unwrap_or_default()
                                    ></div>
                                    <Artwork src=ep.artwork() title=ep.podcast_title.clone() class="modal-art" />
                                    <div class="modal-head">
                                        <button
                                            class="eyebrow link"
                                            on:click=move |_| {
                                                close();
                                                app.navigate(View::Podcast(pid.clone()));
                                            }
                                        >
                                            {ep.podcast_title.clone()}
                                        </button>
                                        <h2>{e.title.clone()}</h2>
                                        <div class="modal-meta">
                                            {move || fmt_date(app.lang(), e.pub_date)}
                                            {e.duration.map(|d| format!(" · {}", fmt_duration(d)))}
                                            {e.is_video().then(|| view! { " · " {move || app.tr("ep.video")} })}
                                            {move || size.map(|s| format!(" · {}", fmt_bytes_lang(app.lang(), s)))}
                                        </div>
                                        <div class="modal-actions">
                                            <button
                                                class="btn primary"
                                                on:click=move |_| player.toggle_episode(ep_play.clone())
                                            >
                                                {move || if playing.get() {
                                                    view! { <Icon name="pause" size=16 /> }
                                                } else {
                                                    view! { <Icon name="play" size=16 /> }
                                                }}
                                                <span>
                                                    {move || app.tr(if playing.get() { "player.pause" } else { "ep.play" })}
                                                </span>
                                            </button>
                                            <DownloadButton ep=ep.clone() />
                                            <EpisodeMenu ep=ep.clone() />
                                        </div>
                                    </div>
                                    <button class="icon-btn modal-close" on:click=move |_| close()>
                                        <Icon name="x" size=20 />
                                    </button>
                                </div>
                                <div class="modal-body">
                                    <div class="notes" inner_html=e.description_html.clone()></div>
                                </div>
                            </div>
                        </div>
                    }
                })
            }}
        </Show>
    }
}
