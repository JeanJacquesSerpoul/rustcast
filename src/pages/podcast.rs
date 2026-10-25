use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::FeedPreview;

use crate::api;
use crate::components::common::{Artwork, EmptyState, Spinner};
use crate::components::episode::EpisodeRow;
use crate::icons::Icon;
use crate::player::player;
use crate::state::{app, async_data, ToastKind, View};

#[derive(Clone, Copy, PartialEq)]
enum Filter {
    All,
    Unplayed,
    Downloaded,
}

const PAGE: usize = 40;

/// État d'affichage conservé quand les données sont rechargées.
#[derive(Clone, Copy)]
struct Ui {
    filter: RwSignal<Filter>,
    search: RwSignal<String>,
    oldest_first: RwSignal<bool>,
    shown: RwSignal<usize>,
    desc_open: RwSignal<bool>,
}

/// Page d'un podcast : `id` pour un abonnement, `feed_url` pour un aperçu.
#[component]
pub fn PodcastPage(#[prop(optional)] id: Option<String>, #[prop(optional)] feed_url: Option<String>) -> impl IntoView {
    let app = app();
    let ui = Ui {
        filter: RwSignal::new(Filter::All),
        search: RwSignal::new(String::new()),
        oldest_first: RwSignal::new(false),
        shown: RwSignal::new(PAGE),
        desc_open: RwSignal::new(false),
    };
    let data = async_data(move || {
        app.library_rev.track();
        let (id, feed_url) = (id.clone(), feed_url.clone());
        async move {
            match (id, feed_url) {
                (Some(id), _) => api::get_podcast(id).await,
                (None, Some(url)) => api::preview_feed(url).await,
                _ => Err("?".into()),
            }
        }
    });

    view! {
        <div class="page podcast-page">
            {move || match data.get() {
                None => view! { <div class="center-fill"><Spinner /></div> }.into_any(),
                Some(Err(e)) => view! {
                    <EmptyState
                        icon="rss"
                        title=Signal::derive(move || app.tr("pod.load_error").to_string())
                        text=e
                    >
                        <button class="btn ghost" on:click=move |_| app.back()>
                            <Icon name="back" size=16 />
                            <span>{move || app.tr("action.back")}</span>
                        </button>
                    </EmptyState>
                }.into_any(),
                Some(Ok(feed)) => view! { <PodcastContent feed=feed ui=ui /> }.into_any(),
            }}
        </div>
    }
}

#[component]
fn PodcastContent(feed: FeedPreview, ui: Ui) -> impl IntoView {
    let app = app();
    let player = player();
    let p = feed.podcast.clone();
    let subscribed = feed.subscribed;
    let episodes = StoredValue::new(feed.episodes.clone());

    let Ui { filter, search, oldest_first, shown, desc_open } = ui;
    let refreshing = RwSignal::new(false);
    let busy = RwSignal::new(false);

    let counts = episodes.with_value(|eps| {
        (
            eps.len(),
            eps.iter().filter(|e| !e.episode.played).count(),
            eps.iter().filter(|e| e.episode.local_path.is_some()).count(),
        )
    });

    let filtered = Memo::new(move |_| {
        let q = search.get().to_lowercase();
        let f = filter.get();
        let mut list: Vec<_> = episodes.with_value(|eps| {
            eps.iter()
                .filter(|e| match f {
                    Filter::All => true,
                    Filter::Unplayed => !e.episode.played,
                    Filter::Downloaded => e.episode.local_path.is_some(),
                })
                .filter(|e| {
                    q.is_empty()
                        || e.episode.title.to_lowercase().contains(&q)
                        || e.episode.summary.to_lowercase().contains(&q)
                })
                .cloned()
                .collect()
        });
        if oldest_first.get() {
            list.reverse();
        }
        list
    });

    let long_desc = p.description.len() > 260;
    let pid = p.id.clone();
    let feed_url = p.feed_url.clone();
    let toggle_sub = move |_| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        let (pid, feed_url) = (pid.clone(), feed_url.clone());
        spawn_local(async move {
            if subscribed {
                if api::unsubscribe(pid).await.is_ok() {
                    app.toast(app.tr("toast.unsubscribed"), ToastKind::Info);
                    app.go_root(View::Library);
                }
            } else {
                match api::subscribe(feed_url).await {
                    Ok(_) => app.toast(app.tr("toast.subscribed_short"), ToastKind::Success),
                    Err(e) => app.error(e),
                }
            }
            busy.set(false);
        });
    };

    let pid_refresh = p.id.clone();
    let refresh = move |_| {
        refreshing.set(true);
        let id = pid_refresh.clone();
        spawn_local(async move {
            match api::refresh_podcast(id).await {
                Ok(0) => app.toast(app.tr("toast.up_to_date"), ToastKind::Info),
                Ok(n) => app.toast(format!("{n} {}", app.tr("toast.new_episodes")), ToastKind::Success),
                Err(e) => app.error(e),
            }
            refreshing.set(false);
        });
    };

    let pid_played = p.id.clone();
    let mark_all = move |_| {
        let id = pid_played.clone();
        spawn_local(async move {
            let _ = api::mark_all_played(id).await;
        });
    };

    // Podcast vidéo si la majorité des épisodes récents sont des vidéos.
    let is_video_show = episodes.with_value(|eps| {
        let recent: Vec<_> = eps.iter().take(20).collect();
        !recent.is_empty() && recent.iter().filter(|e| e.episode.is_video()).count() * 2 > recent.len()
    });
    let latest = episodes.with_value(|e| e.iter().find(|e| !e.episode.played).or(e.first()).cloned());
    let has_latest = latest.is_some();
    let play_latest = move |_| {
        if let Some(ep) = latest.clone() {
            player.play(ep);
        }
    };

    let tab = move |f: Filter, key: &'static str, n: usize| {
        view! {
            <button class:active=move || filter.get() == f on:click=move |_| { filter.set(f); shown.set(PAGE); }>
                {move || app.tr(key)}
                <span class="count">{n}</span>
            </button>
        }
    };

    view! {
        <section class="hero">
            <div
                class="hero-bg"
                style=p.artwork.clone().map(|a| format!("background-image:url('{a}')")).unwrap_or_default()
            ></div>
            <div class="hero-fade"></div>
            <div class="hero-top">
                <Show when=move || !app.history.with(Vec::is_empty)>
                    <button class="icon-btn back-btn" title=move || app.tr("action.back") on:click=move |_| app.back()>
                        <Icon name="back" size=20 />
                    </button>
                </Show>
            </div>
            <div class="hero-content">
                <Artwork src=p.artwork.clone() title=p.title.clone() class="hero-art" />
                <div class="hero-info">
                    <div class="chips">
                        {is_video_show.then(|| view! {
                            <span class="chip video">
                                <Icon name="video" size=12 />
                                {move || app.tr("ep.video")}
                            </span>
                        })}
                        {p.categories.iter().take(3).map(|c| view! { <span class="chip">{c.clone()}</span> }).collect_view()}
                    </div>
                    <h1>{p.title.clone()}</h1>
                    <div class="hero-author">{p.author.clone()}</div>
                    <div class="hero-actions">
                        <Show when=move || has_latest>
                            <button class="btn primary" on:click=play_latest.clone()>
                                <Icon name="play" size=16 />
                                <span>{move || app.tr("pod.play_latest")}</span>
                            </button>
                        </Show>
                        <button
                            class="btn"
                            class:subscribed=subscribed
                            class:ghost=subscribed
                            class:accent=!subscribed
                            class:loading=move || busy.get()
                            on:click=toggle_sub
                        >
                            {if subscribed {
                                view! {
                                    <Icon name="check" size=16 />
                                    <span class="when-idle">{move || app.tr("pod.subscribed")}</span>
                                    <span class="when-hover">{move || app.tr("pod.unsubscribe")}</span>
                                }.into_any()
                            } else {
                                view! {
                                    <Icon name="plus" size=16 />
                                    <span>{move || app.tr("pod.subscribe")}</span>
                                }.into_any()
                            }}
                        </button>
                        {subscribed.then(|| view! {
                            <button
                                class="icon-btn round"
                                class:spinning=move || refreshing.get()
                                title=move || app.tr("action.refresh")
                                on:click=refresh.clone()
                            >
                                <Icon name="refresh" size=18 />
                            </button>
                            <button class="icon-btn round" title=move || app.tr("pod.mark_all") on:click=mark_all.clone()>
                                <Icon name="check-circle" size=18 />
                            </button>
                        })}
                        {p.link.clone().map(|link| view! {
                            <a class="icon-btn round" href=link title=move || app.tr("pod.website")>
                                <Icon name="external" size=18 />
                            </a>
                        })}
                    </div>
                </div>
            </div>
        </section>

        <Show when={
            let d = p.description.clone();
            move || !d.trim().is_empty()
        }>
            <div class="description" class:open=move || { desc_open.get() || !long_desc }>
                <div class="notes" inner_html=p.description.clone()></div>
                {long_desc.then(|| view! {
                    <button class="link more" on:click=move |_| desc_open.update(|o| *o = !*o)>
                        {move || app.tr(if desc_open.get() { "action.less" } else { "action.more" })}
                    </button>
                })}
            </div>
        </Show>

        <div class="toolbar">
            <div class="tabs">
                {tab(Filter::All, "pod.all", counts.0)}
                {tab(Filter::Unplayed, "pod.unplayed", counts.1)}
                {tab(Filter::Downloaded, "pod.downloaded", counts.2)}
            </div>
            <div class="spacer"></div>
            <div class="search-field small">
                <Icon name="search" size=16 />
                <input
                    type="text"
                    placeholder=move || app.tr("pod.search")
                    prop:value=move || search.get()
                    on:input=move |ev| { search.set(event_target_value(&ev)); shown.set(PAGE); }
                />
            </div>
            <button
                class="icon-btn"
                class:active=move || oldest_first.get()
                title=move || app.tr(if oldest_first.get() { "pod.oldest" } else { "pod.newest" })
                on:click=move |_| oldest_first.update(|o| *o = !*o)
            >
                <Icon name="sort" size=18 />
            </button>
        </div>

        {move || {
            let list = filtered.get();
            if list.is_empty() {
                return view! {
                    <EmptyState
                        icon="headphones"
                        title=Signal::derive(move || app.tr("pod.no_match").to_string())
                        text=Signal::derive(move || app.tr("pod.no_match_text").to_string())
                    />
                }.into_any();
            }
            let total = list.len();
            view! {
                <div class="ep-list">
                    {list.into_iter().take(shown.get()).map(|ep| view! { <EpisodeRow ep=ep /> }).collect_view()}
                </div>
                <Show when=move || { shown.get() < total }>
                    <div class="load-more">
                        <button class="btn ghost" on:click=move |_| shown.update(|s| *s += PAGE)>
                            {move || format!("{} ({})", app.tr("action.show_more"), total - shown.get())}
                        </button>
                    </div>
                </Show>
            }.into_any()
        }}
    }
}
