//! Barre latérale, cartes de podcast et notifications.

use leptos::prelude::*;
use shared::{Lang, SearchResult, Theme};

use crate::components::common::Artwork;
use crate::icons::Icon;
use crate::state::{app, ToastKind, View};

#[component]
pub fn Sidebar() -> impl IntoView {
    let app = app();
    let nav = move |icon: &'static str, key: &'static str, target: View| {
        let t2 = target.clone();
        let t3 = target.clone();
        let active = move || {
            let v = app.view.get();
            v == target
                || (matches!(target, View::Library) && matches!(v, View::Podcast(_)))
                || (matches!(target, View::Discover) && matches!(v, View::Preview(_)))
        };
        let badge = Memo::new(move |_| match &t2 {
            View::Queue => app.queue.with(|q| q.len()),
            View::Downloads => app.downloads.with(|d| d.len()),
            _ => 0,
        });
        view! {
            <button class="nav-item" class:active=active on:click=move |_| app.go_root(t3.clone())>
                <Icon name=icon size=20 />
                <span>{move || app.tr(key)}</span>
                <Show when=move || { badge.get() > 0 }>
                    <span class="nav-badge">{badge}</span>
                </Show>
            </button>
        }
    };

    view! {
        <aside class="sidebar">
            <div class="brand">
                <img src="public/logo.svg" alt="" />
                <span>"RustCast"</span>
            </div>
            <nav>
                <div class="nav-label">{move || app.tr("nav.listen")}</div>
                {nav("home", "nav.home", View::Home)}
                {nav("compass", "nav.discover", View::Discover)}
                {nav("library", "nav.library", View::Library)}
                <div class="nav-label">{move || app.tr("nav.mine")}</div>
                {nav("queue", "nav.queue", View::Queue)}
                {nav("download", "nav.downloads", View::Downloads)}
                {nav("settings", "nav.settings", View::Settings)}
            </nav>
            <div class="sidebar-shows">
                <For
                    each=move || app.podcasts.get().into_iter().take(8)
                    key=|p| p.podcast.id.clone()
                    children=move |p| {
                        let id = p.podcast.id.clone();
                        let id2 = id.clone();
                        view! {
                            <button
                                class="mini-show"
                                class:active=move || app.view.get() == View::Podcast(id.clone())
                                title=p.podcast.title.clone()
                                on:click=move |_| app.navigate(View::Podcast(id2.clone()))
                            >
                                <Artwork src=p.podcast.artwork.clone() title=p.podcast.title.clone() />
                                <span>{p.podcast.title.clone()}</span>
                                {(p.unplayed_count > 0).then(|| view! { <i class="unplayed-dot"></i> })}
                            </button>
                        }
                    }
                />
            </div>
            <div class="sidebar-foot">
                <button
                    class="version-link"
                    title=move || app.tr("about.title")
                    on:click=move |_| {
                        app.go_root(View::Settings);
                        // Laisse la page s'afficher avant de faire défiler jusqu'à « À propos ».
                        set_timeout(
                            || {
                                if let Some(el) = document().get_element_by_id("about") {
                                    let opts = web_sys::ScrollIntoViewOptions::new();
                                    opts.set_behavior(web_sys::ScrollBehavior::Instant);
                                    el.scroll_into_view_with_scroll_into_view_options(&opts);
                                }
                            },
                            std::time::Duration::from_millis(60),
                        );
                    }
                >
                    {format!("v{}", crate::util::VERSION)}
                </button>
                <div class="segmented small">
                    <button
                        class:active=move || app.lang() == Lang::Fr
                        on:click=move |_| app.update_settings(|s| s.language = Lang::Fr)
                    >
                        "FR"
                    </button>
                    <button
                        class:active=move || app.lang() == Lang::En
                        on:click=move |_| app.update_settings(|s| s.language = Lang::En)
                    >
                        "EN"
                    </button>
                </div>
                <button
                    class="icon-btn"
                    title=move || app.tr("settings.theme")
                    on:click=move |_| {
                        app.update_settings(|s| {
                            s.theme = if s.theme == Theme::Dark { Theme::Light } else { Theme::Dark }
                        })
                    }
                >
                    {move || if app.settings.with(|s| s.theme == Theme::Dark) {
                        view! { <Icon name="sun" size=18 /> }
                    } else {
                        view! { <Icon name="moon" size=18 /> }
                    }}
                </button>
            </div>
        </aside>
    }
}

/// Carte d'un podcast abonné.
#[component]
pub fn PodcastCard(summary: shared::PodcastSummary) -> impl IntoView {
    let app = app();
    let p = summary.podcast.clone();
    let id = p.id.clone();
    view! {
        <div class="card" on:click=move |_| app.navigate(View::Podcast(id.clone()))>
            <div class="card-art">
                <Artwork src=p.artwork.clone() title=p.title.clone() />
                {(summary.unplayed_count > 0).then(|| view! {
                    <span class="badge">{summary.unplayed_count.min(99)}</span>
                })}
            </div>
            <div class="card-title">{p.title.clone()}</div>
            <div class="card-sub">{p.author.clone()}</div>
        </div>
    }
}

/// Carte d'un résultat de recherche ou du classement.
#[component]
pub fn ResultCard(result: SearchResult, #[prop(optional)] rank: Option<usize>) -> impl IntoView {
    let app = app();
    let feed = result.feed_url.clone().unwrap_or_default();
    let feed_open = feed.clone();
    let feed_sub = feed.clone();
    let subscribed = Memo::new(move |_| app.is_subscribed(&feed));
    let busy = RwSignal::new(false);
    view! {
        <div class="card" on:click=move |_| app.navigate(View::Preview(feed_open.clone()))>
            <div class="card-art">
                <Artwork src=result.artwork.clone() title=result.title.clone() />
                {rank.map(|r| view! { <span class="rank">{r}</span> })}
                <button
                    class="card-sub-btn"
                    class:done=move || subscribed.get()
                    title=move || app.tr(if subscribed.get() { "pod.subscribed" } else { "pod.subscribe" })
                    on:click=move |ev| {
                        ev.stop_propagation();
                        if !subscribed.get_untracked() && !busy.get_untracked() {
                            busy.set(true);
                            app.subscribe(feed_sub.clone());
                        }
                    }
                >
                    {move || if subscribed.get() {
                        view! { <Icon name="check" size=18 /> }
                    } else {
                        view! { <Icon name="plus" size=18 /> }
                    }}
                </button>
            </div>
            <div class="card-title">{result.title.clone()}</div>
            <div class="card-sub">{result.author.clone()}</div>
        </div>
    }
}

#[component]
pub fn Toasts() -> impl IntoView {
    let app = app();
    view! {
        <div class="toasts">
            <For
                each=move || app.toasts.get()
                key=|t| t.id
                children=move |t| {
                    let icon = match t.kind {
                        ToastKind::Success => "check-circle",
                        ToastKind::Error => "info",
                        ToastKind::Info => "sparkles",
                    };
                    let kind = match t.kind {
                        ToastKind::Success => "success",
                        ToastKind::Error => "error",
                        ToastKind::Info => "info",
                    };
                    view! {
                        <div class=format!("toast {kind}")>
                            <Icon name=icon size=18 />
                            <span>{t.msg.clone()}</span>
                        </div>
                    }
                }
            />
        </div>
    }
}

/// Menu d'import / export OPML.
#[component]
pub fn OpmlMenu() -> impl IntoView {
    let app = app();
    let open = RwSignal::new(false);
    view! {
        <div class="menu-wrap">
            <button
                class="btn ghost"
                class:loading=move || app.importing.with(Option::is_some)
                title=move || app.tr("opml.title")
                on:click=move |_| open.update(|o| *o = !*o)
            >
                <Icon name="rss" size=16 />
                <span>"OPML"</span>
                <Icon name="chevron-down" size=14 />
            </button>
            <Show when=move || open.get()>
                <div class="menu-backdrop" on:click=move |_| open.set(false)></div>
                <div class="menu">
                    <button class="menu-item" on:click=move |_| { open.set(false); app.import_opml(); }>
                        <Icon name="upload" size=17 />
                        <div class="menu-text">
                            <span>{move || app.tr("settings.import")}</span>
                            <small>{move || app.tr("settings.import_desc")}</small>
                        </div>
                    </button>
                    <button
                        class="menu-item"
                        class:disabled=move || app.podcasts.with(Vec::is_empty)
                        on:click=move |_| { open.set(false); app.export_opml(); }
                    >
                        <Icon name="download" size=17 />
                        <div class="menu-text">
                            <span>{move || app.tr("settings.export")}</span>
                            <small>{move || app.tr("settings.export_desc")}</small>
                        </div>
                    </button>
                </div>
            </Show>
        </div>
    }
}

/// Bandeau d'avancement d'un import OPML.
#[component]
pub fn ImportBanner() -> impl IntoView {
    let app = app();
    view! {
        <Show when=move || app.importing.with(|p| p.as_ref().is_some_and(|p| p.total > 0))>
            {move || app.importing.get().map(|p| {
                let pct = if p.total > 0 { p.done as f64 / p.total as f64 * 100.0 } else { 0.0 };
                view! {
                    <div class="import-banner">
                        <span class="spinner small"></span>
                        <div class="import-text">
                            <strong>{move || app.tr("opml.importing")}</strong>
                            <span>{format!("{} / {}", p.done, p.total)}</span>
                        </div>
                        <div class="progress"><span style:width=format!("{pct:.1}%")></span></div>
                    </div>
                }
            })}
        </Show>
    }
}
