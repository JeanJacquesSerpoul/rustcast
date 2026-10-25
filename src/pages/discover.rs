use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::SearchResult;

use crate::api;
use crate::components::common::{EmptyState, PageHeader, Spinner};
use crate::components::layout::ResultCard;
use crate::icons::Icon;
use crate::state::{app, async_data, View};

const REGIONS: [(&str, &str); 8] = [
    ("fr", "France"),
    ("be", "Belgique"),
    ("ca", "Canada"),
    ("ch", "Suisse"),
    ("us", "United States"),
    ("gb", "United Kingdom"),
    ("de", "Deutschland"),
    ("es", "España"),
];

// Persiste la dernière recherche pendant la session pour revenir de la page d'un podcast.
thread_local! {
    static LAST_QUERY: std::cell::RefCell<(String, Option<Vec<SearchResult>>)> = Default::default();
}

#[component]
pub fn DiscoverPage() -> impl IntoView {
    let app = app();
    let (q0, r0) = LAST_QUERY.with(|l| l.borrow().clone());
    let query = RwSignal::new(q0);
    let results = RwSignal::new(r0.map(Ok::<_, String>));
    let searching = RwSignal::new(false);
    let region = Signal::derive(move || app.settings.with(|s| s.region.clone()));

    let top = async_data(move || api::top_podcasts(region.get()));

    let run_search = move || {
        let term = query.get_untracked().trim().to_string();
        if term.is_empty() {
            results.set(None);
            LAST_QUERY.with(|l| *l.borrow_mut() = Default::default());
            return;
        }
        searching.set(true);
        spawn_local(async move {
            let r = api::search_podcasts(term.clone(), region.get_untracked()).await;
            LAST_QUERY.with(|l| *l.borrow_mut() = (term, r.clone().ok()));
            // Une URL de flux collée : on ouvre directement l'aperçu.
            if let Ok(list) = &r {
                if term_is_url(&query.get_untracked()) {
                    if let Some(feed) = list.first().and_then(|x| x.feed_url.clone()) {
                        app.navigate(View::Preview(feed));
                    }
                }
            }
            results.set(Some(r));
            searching.set(false);
        });
    };

    view! {
        <div class="page">
            <PageHeader
                title=Signal::derive(move || app.tr("nav.discover").to_string())
                subtitle=Signal::derive(move || app.tr("disc.subtitle").to_string())
            />
            <form
                class="search-hero"
                on:submit=move |ev| {
                    ev.prevent_default();
                    run_search();
                }
            >
                <div class="search-field large">
                    <Icon name="search" size=20 />
                    <input
                        type="text"
                        autofocus
                        placeholder=move || app.tr("disc.placeholder")
                        prop:value=move || query.get()
                        on:input=move |ev| {
                            query.set(event_target_value(&ev));
                            if query.get_untracked().is_empty() {
                                run_search();
                            }
                        }
                    />
                    <Show when=move || !query.get().is_empty()>
                        <button
                            type="button"
                            class="icon-btn"
                            on:click=move |_| {
                                query.set(String::new());
                                run_search();
                            }
                        >
                            <Icon name="x" size=16 />
                        </button>
                    </Show>
                </div>
                <button type="submit" class="btn primary large">
                    {move || if searching.get() { view! { <span class="spinner small"></span> }.into_any() } else { view! { <Icon name="search" size=18 /> }.into_any() }}
                    <span>{move || app.tr("disc.search")}</span>
                </button>
            </form>
            <p class="hint">
                <Icon name="rss" size=14 />
                <span>{move || app.tr("disc.hint")}</span>
            </p>

            {move || match results.get() {
                Some(Ok(list)) => view! {
                    <section class="section">
                        <h2 class="section-title">
                            {move || app.tr("disc.results")}
                            <span class="count">{list.len()}</span>
                        </h2>
                        {if list.is_empty() {
                            view! {
                                <EmptyState
                                    icon="search"
                                    title=Signal::derive(move || app.tr("disc.no_results").to_string())
                                    text=Signal::derive(move || app.tr("disc.no_results_text").to_string())
                                />
                            }.into_any()
                        } else {
                            view! {
                                <div class="grid">
                                    {list.into_iter().map(|r| view! { <ResultCard result=r /> }).collect_view()}
                                </div>
                            }.into_any()
                        }}
                    </section>
                }.into_any(),
                Some(Err(e)) => view! { <p class="error-text">{e}</p> }.into_any(),
                None => view! {
                    <section class="section">
                        <div class="section-head">
                            <h2 class="section-title">
                                <Icon name="sparkles" size=18 />
                                {move || app.tr("disc.top")}
                            </h2>
                            <select
                                class="select"
                                prop:value=move || region.get()
                                on:change=move |ev| {
                                    let v = event_target_value(&ev);
                                    app.update_settings(|s| s.region = v);
                                }
                            >
                                {REGIONS.iter().map(|(code, name)| view! {
                                    <option value=*code selected=move || region.get() == *code>{*name}</option>
                                }).collect_view()}
                            </select>
                        </div>
                        {move || match top.get() {
                            None => view! { <Spinner /> }.into_any(),
                            Some(Err(e)) => view! { <p class="error-text">{e}</p> }.into_any(),
                            Some(Ok(list)) => view! {
                                <div class="grid">
                                    {list.into_iter().enumerate().map(|(i, r)| view! { <ResultCard result=r rank=i + 1 /> }).collect_view()}
                                </div>
                            }.into_any(),
                        }}
                    </section>
                }.into_any(),
            }}
        </div>
    }
}

fn term_is_url(s: &str) -> bool {
    s.trim().contains("://")
}
