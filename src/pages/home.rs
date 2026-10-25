use leptos::prelude::*;

use crate::api;
use crate::components::common::{Artwork, EmptyState, PageHeader, Spinner};
use crate::components::episode::EpisodeRow;
use crate::icons::Icon;
use crate::player::player;
use crate::state::{app, async_data, View};
use crate::util::fmt_remaining;

fn greeting_key() -> &'static str {
    match js_sys::Date::new_0().get_hours() {
        5..=11 => "home.morning",
        12..=17 => "home.afternoon",
        _ => "home.evening",
    }
}

#[component]
pub fn HomePage() -> impl IntoView {
    let app = app();
    let player = player();
    let latest = async_data(move || {
        app.library_rev.track();
        api::get_latest(40)
    });
    let in_progress = async_data(move || {
        app.library_rev.track();
        api::get_in_progress()
    });
    let has_podcasts = move || !app.podcasts.with(Vec::is_empty);

    view! {
        <div class="page">
            <PageHeader
                title=Signal::derive(move || app.tr(greeting_key()).to_string())
                subtitle=Signal::derive(move || app.tr("home.subtitle").to_string())
            >
                <Show when=has_podcasts>
                    <button
                        class="btn ghost"
                        class:spinning=move || app.refreshing.get()
                        on:click=move |_| app.refresh_all()
                    >
                        <Icon name="refresh" size=16 />
                        <span>{move || app.tr("action.refresh")}</span>
                    </button>
                </Show>
            </PageHeader>

            <Show
                when=move || has_podcasts() || !app.ready.get()
                fallback=move || view! {
                    <div class="welcome">
                        <div class="welcome-glow"></div>
                        <img src="public/logo.svg" alt="" class="welcome-logo" />
                        <h2>{move || app.tr("home.welcome_title")}</h2>
                        <p>{move || app.tr("home.welcome_text")}</p>
                        <div class="empty-actions">
                            <button class="btn primary large" on:click=move |_| app.go_root(View::Discover)>
                                <Icon name="compass" size=18 />
                                <span>{move || app.tr("home.welcome_cta")}</span>
                            </button>
                            <button class="btn ghost large" on:click=move |_| app.import_opml()>
                                <Icon name="upload" size=18 />
                                <span>{move || app.tr("opml.import_cta")}</span>
                            </button>
                        </div>
                        <p class="welcome-hint">{move || app.tr("opml.hint")}</p>
                    </div>
                }
            >
                {move || {
                    in_progress.get().and_then(Result::ok).filter(|l| !l.is_empty()).map(|list| view! {
                        <section class="section">
                            <h2 class="section-title">{move || app.tr("home.continue")}</h2>
                            <div class="resume-row">
                                {list.into_iter().map(|ep| {
                                    let e = ep.episode.clone();
                                    let id = e.id.clone();
                                    let ep_click = ep.clone();
                                    let live = Memo::new(move |_| player.is_current(&id));
                                    let pos = move || if live.get() { player.time.get() } else { e.position };
                                    let total = e.duration.unwrap_or(0) as f64;
                                    let ratio = move || if total > 0.0 { (pos() / total).min(1.0) } else { 0.0 };
                                    view! {
                                        <button class="resume-card" on:click=move |_| player.toggle_episode(ep_click.clone())>
                                            <div class="resume-art">
                                                <Artwork src=ep.artwork() title=ep.podcast_title.clone() />
                                                <span class="resume-play">
                                                    {move || if live.get() && player.playing.get() {
                                                        view! { <Icon name="pause" size=18 /> }
                                                    } else {
                                                        view! { <Icon name="play" size=18 /> }
                                                    }}
                                                </span>
                                            </div>
                                            <div class="resume-info">
                                                <div class="resume-podcast">{ep.podcast_title.clone()}</div>
                                                <div class="resume-title">{e.title.clone()}</div>
                                                <div class="resume-bar"><span style:width=move || format!("{:.1}%", ratio() * 100.0)></span></div>
                                                <div class="resume-left">
                                                    {move || if total > 0.0 { fmt_remaining(app.lang(), total - pos()) } else { String::new() }}
                                                </div>
                                            </div>
                                        </button>
                                    }
                                }).collect_view()}
                            </div>
                        </section>
                    })
                }}

                <section class="section">
                    <h2 class="section-title">{move || app.tr("home.latest")}</h2>
                    {move || match latest.get() {
                        None => view! { <Spinner /> }.into_any(),
                        Some(Err(e)) => view! { <p class="error-text">{e}</p> }.into_any(),
                        Some(Ok(list)) if list.is_empty() => view! {
                            <EmptyState
                                icon="rss"
                                title=Signal::derive(move || app.tr("home.no_episodes").to_string())
                                text=Signal::derive(move || app.tr("home.no_episodes_text").to_string())
                            />
                        }.into_any(),
                        Some(Ok(list)) => view! {
                            <div class="ep-list">
                                {list.into_iter().map(|ep| view! { <EpisodeRow ep=ep show_podcast=true /> }).collect_view()}
                            </div>
                        }.into_any(),
                    }}
                </section>
            </Show>
        </div>
    }
}
