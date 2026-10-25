use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;
use crate::components::common::{Artwork, EmptyState, Equalizer, PageHeader};
use crate::icons::Icon;
use crate::player::player;
use crate::state::{app, View};
use crate::util::{fmt_clock, fmt_duration};

#[component]
pub fn QueuePage() -> impl IntoView {
    let app = app();
    let player = player();
    let dragging = RwSignal::new(None::<usize>);
    let over = RwSignal::new(None::<usize>);

    let save = move |ids: Vec<String>| {
        spawn_local(async move {
            let _ = api::set_queue(ids).await;
        });
    };
    let move_item = move |from: usize, to: usize| {
        let mut ids: Vec<String> = app.queue.with_untracked(|q| q.iter().map(|e| e.episode.id.clone()).collect());
        if from >= ids.len() || to >= ids.len() || from == to {
            return;
        }
        let id = ids.remove(from);
        ids.insert(to, id);
        // Mise à jour immédiate pour un retour visuel fluide.
        app.queue.update(|q| {
            let e = q.remove(from);
            q.insert(to, e);
        });
        save(ids);
    };

    let total = move || {
        app.queue.with(|q| q.iter().filter_map(|e| e.episode.duration).map(u64::from).sum::<u64>())
    };

    view! {
        <div class="page">
            <PageHeader
                title=Signal::derive(move || app.tr("nav.queue").to_string())
                subtitle=Signal::derive(move || {
                    let n = app.queue.with(Vec::len);
                    if n == 0 {
                        app.tr("queue.subtitle").to_string()
                    } else {
                        format!("{n} {} · {}", app.tr(if n > 1 { "queue.episodes" } else { "queue.episode" }), fmt_duration(total() as u32))
                    }
                })
            >
                <Show when=move || !app.queue.with(Vec::is_empty)>
                    <button class="btn ghost" on:click=move |_| save(vec![])>
                        <Icon name="trash" size=16 />
                        <span>{move || app.tr("queue.clear")}</span>
                    </button>
                </Show>
            </PageHeader>

            {move || player.current.get().map(|ep| {
                let ep2 = ep.clone();
                view! {
                    <section class="section">
                        <h2 class="section-title">{move || app.tr("queue.now")}</h2>
                        <div class="now-card" on:click=move |_| app.now_playing_open.set(true)>
                            <Artwork src=ep.artwork() title=ep.podcast_title.clone() />
                            <div class="now-info">
                                <div class="now-podcast">{ep.podcast_title.clone()}</div>
                                <div class="now-title">{ep.episode.title.clone()}</div>
                                <div class="now-time">
                                    <Equalizer active=player.playing />
                                    {move || format!("{} / {}", fmt_clock(player.time.get()), fmt_clock(player.duration.get()))}
                                </div>
                            </div>
                            <button
                                class="play-main"
                                on:click=move |ev| {
                                    ev.stop_propagation();
                                    player.toggle_episode(ep2.clone());
                                }
                            >
                                {move || if player.playing.get() {
                                    view! { <Icon name="pause" size=22 /> }
                                } else {
                                    view! { <Icon name="play" size=22 /> }
                                }}
                            </button>
                        </div>
                    </section>
                }
            })}

            <section class="section">
                <h2 class="section-title">{move || app.tr("queue.up_next")}</h2>
                <Show
                    when=move || !app.queue.with(Vec::is_empty)
                    fallback=move || view! {
                        <EmptyState
                            icon="queue"
                            title=Signal::derive(move || app.tr("queue.empty").to_string())
                            text=Signal::derive(move || app.tr("queue.empty_text").to_string())
                        />
                    }
                >
                    <div class="queue-list">
                        {move || {
                            let q = app.queue.get();
                            let n = q.len();
                            q.into_iter().enumerate().map(|(i, ep)| {
                                let id = ep.episode.id.clone();
                                let ep_play = ep.clone();
                                let pid = ep.episode.podcast_id.clone();
                                view! {
                                    <div
                                        class="queue-item"
                                        class:dragging=move || dragging.get() == Some(i)
                                        class:drop-target=move || over.get() == Some(i) && dragging.get() != Some(i)
                                        draggable="true"
                                        on:dragstart=move |_| dragging.set(Some(i))
                                        on:dragend=move |_| { dragging.set(None); over.set(None); }
                                        on:dragover=move |ev| { ev.prevent_default(); over.set(Some(i)); }
                                        on:drop=move |ev| {
                                            ev.prevent_default();
                                            if let Some(from) = dragging.get_untracked() {
                                                move_item(from, i);
                                            }
                                            dragging.set(None);
                                            over.set(None);
                                        }
                                    >
                                        <span class="grip">{i + 1}</span>
                                        <Artwork src=ep.artwork() title=ep.podcast_title.clone() />
                                        <div class="queue-info">
                                            <button class="queue-podcast link" on:click=move |_| app.navigate(View::Podcast(pid.clone()))>
                                                {ep.podcast_title.clone()}
                                            </button>
                                            <div class="queue-title">{ep.episode.title.clone()}</div>
                                        </div>
                                        <span class="queue-dur">{ep.episode.duration.map(fmt_duration)}</span>
                                        <div class="queue-actions">
                                            <button class="icon-btn" title=move || app.tr("player.play") on:click=move |_| player.play(ep_play.clone())>
                                                <Icon name="play" size=16 />
                                            </button>
                                            <button class="icon-btn" class:disabled=i == 0 title=move || app.tr("queue.up") on:click=move |_| { if i > 0 { move_item(i, i - 1) } }>
                                                <Icon name="arrow-up" size=16 />
                                            </button>
                                            <button class="icon-btn" class:disabled=i + 1 == n title=move || app.tr("queue.down") on:click=move |_| move_item(i, i + 1)>
                                                <Icon name="arrow-down" size=16 />
                                            </button>
                                            <button
                                                class="icon-btn"
                                                title=move || app.tr("queue.remove")
                                                on:click=move |_| {
                                                    let id = id.clone();
                                                    spawn_local(async move { let _ = api::queue_remove(id).await; });
                                                }
                                            >
                                                <Icon name="x" size=16 />
                                            </button>
                                        </div>
                                    </div>
                                }
                            }).collect_view()
                        }}
                    </div>
                </Show>
            </section>
        </div>
    }
}
