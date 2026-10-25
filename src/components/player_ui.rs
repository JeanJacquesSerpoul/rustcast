//! Barre de lecture et vue plein écran « En lecture ».

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::components::common::{Artwork, Equalizer};
use crate::components::episode::EpisodeMenu;
use crate::icons::{Icon, SkipIcon};
use crate::player::{self as engine, player, Player};
use crate::state::{app, View};
use crate::util::fmt_clock;

/// Emplacement où s'affiche l'élément vidéo du lecteur ; il le rend au conteneur caché en disparaissant.
#[component]
fn VideoSurface(player: Player, #[prop(optional, into)] class: String, #[prop(default = true)] interactive: bool) -> impl IntoView {
    let host = NodeRef::<leptos::html::Div>::new();
    let attached = StoredValue::new(send_wrapper::SendWrapper::new(None::<web_sys::Element>));
    host.on_load(move |el| {
        let el: web_sys::Element = el.into();
        engine::attach_video(&el);
        attached.set_value(send_wrapper::SendWrapper::new(Some(el)));
    });
    on_cleanup(move || {
        if let Some(wrapper) = attached.try_get_value() {
            if let Some(el) = wrapper.take() {
                engine::detach_video(&el);
            }
        }
    });
    view! {
        <div
            class=format!("video-surface {class}")
            node_ref=host
            on:click=move |_| if interactive { player.toggle() }
            on:dblclick=move |_| if interactive { player.toggle_fullscreen() }
        ></div>
    }
}

const RATES: [f64; 9] = [0.75, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5];

fn fmt_rate(r: f64) -> String {
    let s = format!("{r:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    format!("{s}×")
}

fn input_value(ev: &leptos::ev::Event) -> f64 {
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        .and_then(|i| i.value().parse().ok())
        .unwrap_or(0.0)
}

/// Barre de progression déplaçable, partagée par la barre de lecture et la vue plein écran.
#[component]
fn SeekBar(player: Player, #[prop(default = false)] large: bool) -> impl IntoView {
    let dragging = RwSignal::new(None::<f64>);
    let shown = move || dragging.get().unwrap_or_else(|| player.time.get());
    let max = move || player.duration.get().max(1.0);
    let pct = move || (shown() / max() * 100.0).clamp(0.0, 100.0);
    view! {
        <div class="seek" class:large=large>
            <span class="time">{move || fmt_clock(shown())}</span>
            <input
                type="range"
                min="0"
                step="1"
                prop:max=max
                prop:value=shown
                style=move || format!("--p:{:.2}%", pct())
                on:input=move |ev| dragging.set(Some(input_value(&ev)))
                on:change=move |ev| {
                    player.seek(input_value(&ev));
                    dragging.set(None);
                }
            />
            <span class="time right">
                {move || {
                    let d = player.duration.get();
                    if d > 0.0 { format!("-{}", fmt_clock(d - shown())) } else { "--:--".into() }
                }}
            </span>
        </div>
    }
}

#[component]
fn RateMenu(player: Player, #[prop(default = false)] up: bool) -> impl IntoView {
    let app = app();
    let open = RwSignal::new(false);
    view! {
        <div class="menu-wrap">
            <button
                class="rate-btn"
                class:active=move || { (player.rate.get() - 1.0).abs() > 0.01 }
                title=move || app.tr("player.speed")
                on:click=move |_| open.update(|o| *o = !*o)
            >
                {move || fmt_rate(player.rate.get())}
            </button>
            <Show when=move || open.get()>
                <div class="menu-backdrop" on:click=move |_| open.set(false)></div>
                <div class="menu rate-menu" class:up=up>
                    <div class="menu-title">{move || app.tr("player.speed")}</div>
                    {RATES
                        .iter()
                        .map(|&r| {
                            view! {
                                <button
                                    class="menu-item"
                                    class:selected=move || { (player.rate.get() - r).abs() < 0.01 }
                                    on:click=move |_| {
                                        player.set_rate(r);
                                        open.set(false);
                                    }
                                >
                                    <span>{fmt_rate(r)}</span>
                                    <Show when=move || { (player.rate.get() - r).abs() < 0.01 }>
                                        <Icon name="check" size=16 />
                                    </Show>
                                </button>
                            }
                        })
                        .collect_view()}
                </div>
            </Show>
        </div>
    }
}

#[component]
fn Volume(player: Player) -> impl IntoView {
    let level = move || if player.muted.get() { 0.0 } else { player.volume.get() };
    view! {
        <div class="volume">
            <button class="icon-btn" on:click=move |_| player.toggle_mute()>
                {move || if level() <= 0.001 {
                    view! { <Icon name="volume-x" size=19 /> }
                } else {
                    view! { <Icon name="volume" size=19 /> }
                }}
            </button>
            <input
                type="range"
                min="0"
                max="1"
                step="0.01"
                prop:value=level
                style=move || format!("--p:{:.1}%", level() * 100.0)
                on:input=move |ev| player.set_volume(input_value(&ev))
            />
        </div>
    }
}

#[component]
fn Transport(player: Player, #[prop(default = false)] large: bool) -> impl IntoView {
    let app = app();
    let skip_back = Signal::derive(move || app.settings.with(|s| s.skip_back));
    let skip_fwd = Signal::derive(move || app.settings.with(|s| s.skip_forward));
    let has_next = move || !app.queue.with(Vec::is_empty);
    view! {
        <div class="transport" class:large=large>
            <button class="icon-btn skip" title=move || app.tr("player.back") on:click=move |_| player.skip_back()>
                <SkipIcon seconds=skip_back forward=false size=if large { 34 } else { 26 } />
            </button>
            <button
                class="play-main"
                class:buffering=move || player.buffering.get()
                title=move || app.tr(if player.playing.get() { "player.pause" } else { "player.play" })
                on:click=move |_| player.toggle()
            >
                {move || if player.playing.get() {
                    view! { <Icon name="pause" size=if large { 30 } else { 22 } /> }
                } else {
                    view! { <Icon name="play" size=if large { 30 } else { 22 } /> }
                }}
            </button>
            <button class="icon-btn skip" title=move || app.tr("player.forward") on:click=move |_| player.skip_forward()>
                <SkipIcon seconds=skip_fwd forward=true size=if large { 34 } else { 26 } />
            </button>
            {large.then(|| view! {
                <button
                    class="icon-btn"
                    class:disabled=move || !has_next()
                    title=move || app.tr("player.next")
                    on:click=move |_| player.play_next()
                >
                    <Icon name="next" size=22 />
                </button>
            })}
        </div>
    }
}

#[component]
pub fn PlayerBar() -> impl IntoView {
    let app = app();
    let player = player();
    view! {
        <Show when=move || player.current.with(Option::is_some)>
            {move || player.current.get().map(|ep| {
                let pid = ep.episode.podcast_id.clone();
                view! {
                    <footer class="player-bar">
                        <div class="pb-left">
                            <button
                                class="pb-art"
                                class:video=move || player.has_video.get()
                                title=move || app.tr("player.expand")
                                on:click=move |_| app.now_playing_open.set(true)
                            >
                                {
                                    let (art, title) = (ep.artwork(), ep.podcast_title.clone());
                                    move || if player.has_video.get() && !app.now_playing_open.get() {
                                        view! { <VideoSurface player=player class="pb-video" interactive=false /> }.into_any()
                                    } else {
                                        view! { <Artwork src=art.clone() title=title.clone() /> }.into_any()
                                    }
                                }
                                <span class="pb-art-hover"><Icon name="maximize" size=18 /></span>
                            </button>
                            <div class="pb-info">
                                <div class="pb-title" title=ep.episode.title.clone()>
                                    <Equalizer active=player.playing />
                                    <span>{ep.episode.title.clone()}</span>
                                </div>
                                <button class="pb-podcast link" on:click=move |_| app.navigate(View::Podcast(pid.clone()))>
                                    {ep.podcast_title.clone()}
                                </button>
                            </div>
                        </div>
                        <div class="pb-center">
                            <Transport player=player />
                            <SeekBar player=player />
                        </div>
                        <div class="pb-right">
                            <RateMenu player=player up=true />
                            <Volume player=player />
                            <button
                                class="icon-btn"
                                class:active=move || app.view.get() == View::Queue
                                title=move || app.tr("nav.queue")
                                on:click=move |_| app.go_root(View::Queue)
                            >
                                <Icon name="queue" size=19 />
                            </button>
                            <button class="icon-btn" title=move || app.tr("player.expand") on:click=move |_| app.now_playing_open.set(true)>
                                <Icon name="chevron-up" size=20 />
                            </button>
                        </div>
                    </footer>
                }
            })}
        </Show>
    }
}

#[component]
pub fn NowPlaying() -> impl IntoView {
    let app = app();
    let player = player();
    view! {
        <Show when=move || app.now_playing_open.get() && player.current.with(Option::is_some)>
            {move || player.current.get().map(|ep| {
                let art = ep.artwork();
                let pid = ep.episode.podcast_id.clone();
                view! {
                    <div class="now-playing">
                        <div
                            class="np-bg"
                            style=art.clone().map(|a| format!("background-image:url('{a}')")).unwrap_or_default()
                        ></div>
                        <div class="np-shade"></div>
                        <button class="icon-btn np-close" title=move || app.tr("player.collapse") on:click=move |_| app.now_playing_open.set(false)>
                            <Icon name="chevron-down" size=26 />
                        </button>
                        <div class="np-content" class:with-video=move || player.has_video.get()>
                            <div class="np-main">
                                {
                                    let (art, title) = (art.clone(), ep.podcast_title.clone());
                                    move || if player.has_video.get() {
                                        view! {
                                            <div class="np-video-wrap">
                                                <VideoSurface player=player class="np-video" />
                                                <button
                                                    class="icon-btn np-fullscreen"
                                                    title=move || app.tr("player.fullscreen")
                                                    on:click=move |_| player.toggle_fullscreen()
                                                >
                                                    <Icon name="maximize" size=18 />
                                                </button>
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <div class="np-art" class:playing=move || player.playing.get()>
                                                <Artwork src=art.clone() title=title.clone() />
                                            </div>
                                        }.into_any()
                                    }
                                }
                                <button
                                    class="np-podcast link"
                                    on:click=move |_| app.navigate(View::Podcast(pid.clone()))
                                >
                                    {ep.podcast_title.clone()}
                                </button>
                                <h1 class="np-title">{ep.episode.title.clone()}</h1>
                                <SeekBar player=player large=true />
                                <Transport player=player large=true />
                                <div class="np-extras">
                                    <RateMenu player=player up=true />
                                    <Volume player=player />
                                    <Show when=move || player.has_video.get()>
                                        <button
                                            class="icon-btn"
                                            title=move || app.tr("player.fullscreen")
                                            on:click=move |_| player.toggle_fullscreen()
                                        >
                                            <Icon name="maximize" size=19 />
                                        </button>
                                    </Show>
                                    <EpisodeMenu ep=ep.clone() up=true />
                                </div>
                            </div>
                            <div class="np-notes">
                                <h3>{move || app.tr("ep.notes")}</h3>
                                <div class="notes" inner_html=ep.episode.description_html.clone()></div>
                                <Show when=move || !app.queue.with(Vec::is_empty)>
                                    <h3 class="np-upnext">{move || app.tr("queue.up_next")}</h3>
                                    <div class="np-queue">
                                        {move || app.queue.get().into_iter().take(4).map(|q| {
                                            let q2 = q.clone();
                                            view! {
                                                <button class="np-queue-item" on:click=move |_| player.play(q2.clone())>
                                                    <Artwork src=q.artwork() title=q.podcast_title.clone() />
                                                    <div class="np-q-info">
                                                        <div class="t">{q.episode.title.clone()}</div>
                                                        <div class="s">{q.podcast_title.clone()}</div>
                                                    </div>
                                                    <Icon name="play" size=14 />
                                                </button>
                                            }
                                        }).collect_view()}
                                    </div>
                                </Show>
                            </div>
                        </div>
                    </div>
                }
            })}
        </Show>
    }
}
