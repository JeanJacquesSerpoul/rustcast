use leptos::prelude::*;

use crate::components::common::{EmptyState, PageHeader};
use crate::components::layout::{OpmlMenu, PodcastCard};
use crate::icons::Icon;
use crate::state::{app, View};

#[derive(Clone, Copy, PartialEq)]
enum Sort {
    Recent,
    Title,
    Added,
}

#[component]
pub fn LibraryPage() -> impl IntoView {
    let app = app();
    let sort = RwSignal::new(Sort::Recent);
    let filter = RwSignal::new(String::new());

    let list = Memo::new(move |_| {
        let q = filter.get().to_lowercase();
        let mut l: Vec<_> = app
            .podcasts
            .get()
            .into_iter()
            .filter(|p| {
                q.is_empty()
                    || p.podcast.title.to_lowercase().contains(&q)
                    || p.podcast.author.to_lowercase().contains(&q)
            })
            .collect();
        match sort.get() {
            Sort::Recent => l.sort_by(|a, b| b.latest_pub.cmp(&a.latest_pub)),
            Sort::Title => l.sort_by(|a, b| a.podcast.title.to_lowercase().cmp(&b.podcast.title.to_lowercase())),
            Sort::Added => l.sort_by(|a, b| b.podcast.added_at.cmp(&a.podcast.added_at)),
        }
        l
    });

    let count = move || {
        let n = app.podcasts.with(Vec::len);
        format!("{n} {}", app.tr(if n > 1 { "lib.shows" } else { "lib.show" }))
    };

    view! {
        <div class="page">
            <PageHeader title=Signal::derive(move || app.tr("nav.library").to_string()) subtitle=Signal::derive(count)>
                <div class="search-field small">
                    <Icon name="search" size=16 />
                    <input
                        type="text"
                        placeholder=move || app.tr("lib.filter")
                        prop:value=move || filter.get()
                        on:input=move |ev| filter.set(event_target_value(&ev))
                    />
                </div>
                <div class="segmented">
                    <button class:active=move || sort.get() == Sort::Recent on:click=move |_| sort.set(Sort::Recent)>
                        {move || app.tr("lib.sort_recent")}
                    </button>
                    <button class:active=move || sort.get() == Sort::Title on:click=move |_| sort.set(Sort::Title)>
                        {move || app.tr("lib.sort_title")}
                    </button>
                    <button class:active=move || sort.get() == Sort::Added on:click=move |_| sort.set(Sort::Added)>
                        {move || app.tr("lib.sort_added")}
                    </button>
                </div>
                <OpmlMenu />
            </PageHeader>
            <Show
                when=move || !app.podcasts.with(Vec::is_empty)
                fallback=move || view! {
                    <EmptyState
                        icon="library"
                        title=Signal::derive(move || app.tr("lib.empty").to_string())
                        text=Signal::derive(move || app.tr("lib.empty_text").to_string())
                    >
                        <div class="empty-actions">
                            <button class="btn primary" on:click=move |_| app.go_root(View::Discover)>
                                <Icon name="compass" size=16 />
                                <span>{move || app.tr("home.welcome_cta")}</span>
                            </button>
                            <button class="btn ghost" on:click=move |_| app.import_opml()>
                                <Icon name="upload" size=16 />
                                <span>{move || app.tr("opml.import_cta")}</span>
                            </button>
                        </div>
                    </EmptyState>
                }
            >
                <div class="grid">
                    <For
                        each=move || list.get()
                        key=|p| (p.podcast.id.clone(), p.unplayed_count, p.podcast.artwork.clone())
                        children=move |p| view! { <PodcastCard summary=p /> }
                    />
                </div>
            </Show>
        </div>
    }
}
