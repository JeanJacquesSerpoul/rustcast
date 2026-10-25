use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::DownloadStatus;

use crate::api;
use crate::components::common::{EmptyState, PageHeader, Spinner};
use crate::components::episode::EpisodeRow;
use crate::icons::Icon;
use crate::state::{app, async_data, View};
use crate::util::fmt_bytes_lang;

#[component]
pub fn DownloadsPage() -> impl IntoView {
    let app = app();
    let done = async_data(move || {
        app.library_rev.track();
        api::get_downloaded()
    });
    let storage = async_data(move || {
        app.library_rev.track();
        api::storage_info()
    });

    let active = Memo::new(move |_| {
        let mut l: Vec<_> = app.downloads.get().into_values().collect();
        l.sort_by(|a, b| a.title.cmp(&b.title));
        l
    });

    let subtitle = Signal::derive(move || match storage.get() {
        Some(Ok(s)) => format!(
            "{} {} · {}",
            s.downloads_count,
            app.tr(if s.downloads_count > 1 { "dl.files" } else { "dl.file" }),
            fmt_bytes_lang(app.lang(), s.downloads_bytes)
        ),
        _ => String::new(),
    });

    view! {
        <div class="page">
            <PageHeader title=Signal::derive(move || app.tr("nav.downloads").to_string()) subtitle=subtitle>
                <button
                    class="btn ghost"
                    on:click=move |_| spawn_local(async move { let _ = api::open_data_dir().await; })
                >
                    <Icon name="folder" size=16 />
                    <span>{move || app.tr("dl.open_folder")}</span>
                </button>
            </PageHeader>

            <Show when=move || !active.with(Vec::is_empty)>
                <section class="section">
                    <h2 class="section-title">{move || app.tr("dl.active")}</h2>
                    <div class="dl-active">
                        <For
                            each=move || active.get()
                            key=|d| d.episode_id.clone()
                            children=move |d| {
                                let id = d.episode_id.clone();
                                let id_cancel = id.clone();
                                let id_retry = id.clone();
                                let item = Memo::new(move |_| app.downloads.with(|m| m.get(&id).cloned()));
                                view! {
                                    <div class="dl-item">
                                        <div class="dl-info">
                                            <div class="dl-title">{d.title.clone()}</div>
                                            <div class="dl-sub">
                                                {move || item.get().map(|p| match &p.status {
                                                    DownloadStatus::Queued => app.tr("dl.queued").to_string(),
                                                    DownloadStatus::Failed(e) => format!("{} — {e}", app.tr("dl.failed")),
                                                    _ => match p.total {
                                                        Some(t) => format!(
                                                            "{} / {} · {:.0} %",
                                                            fmt_bytes_lang(app.lang(), p.downloaded),
                                                            fmt_bytes_lang(app.lang(), t),
                                                            p.ratio() * 100.0
                                                        ),
                                                        None => fmt_bytes_lang(app.lang(), p.downloaded),
                                                    },
                                                })}
                                            </div>
                                            <div class="progress">
                                                <span style:width=move || format!("{:.1}%", item.get().map(|p| p.ratio()).unwrap_or(0.0) * 100.0)></span>
                                            </div>
                                        </div>
                                        {move || item.get().is_some_and(|p| matches!(p.status, DownloadStatus::Failed(_))).then(|| {
                                            let id = id_retry.clone();
                                            view! {
                                                <button class="icon-btn" title=app.tr("dl.retry") on:click=move |_| app.download(id.clone())>
                                                    <Icon name="refresh" size=18 />
                                                </button>
                                            }
                                        })}
                                        <button
                                            class="icon-btn"
                                            title=move || app.tr("dl.cancel")
                                            on:click=move |_| {
                                                let id = id_cancel.clone();
                                                spawn_local(async move {
                                                    let _ = api::cancel_download(id.clone()).await;
                                                    app.downloads.update(|m| { m.remove(&id); });
                                                });
                                            }
                                        >
                                            <Icon name="x" size=18 />
                                        </button>
                                    </div>
                                }
                            }
                        />
                    </div>
                </section>
            </Show>

            <section class="section">
                <h2 class="section-title">{move || app.tr("dl.available")}</h2>
                {move || match done.get() {
                    None => view! { <Spinner /> }.into_any(),
                    Some(Err(e)) => view! { <p class="error-text">{e}</p> }.into_any(),
                    Some(Ok(list)) if list.is_empty() => view! {
                        <EmptyState
                            icon="download"
                            title=Signal::derive(move || app.tr("dl.empty").to_string())
                            text=Signal::derive(move || app.tr("dl.empty_text").to_string())
                        >
                            <button class="btn ghost" on:click=move |_| app.go_root(View::Library)>
                                <Icon name="library" size=16 />
                                <span>{move || app.tr("nav.library")}</span>
                            </button>
                        </EmptyState>
                    }.into_any(),
                    Some(Ok(list)) => view! {
                        <div class="ep-list">
                            {list.into_iter().map(|ep| view! { <EpisodeRow ep=ep show_podcast=true /> }).collect_view()}
                        </div>
                    }.into_any(),
                }}
            </section>
        </div>
    }
}
