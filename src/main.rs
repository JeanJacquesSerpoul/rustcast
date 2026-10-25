mod api;
mod components;
mod i18n;
mod icons;
mod pages;
mod player;
mod state;
mod util;

use leptos::prelude::*;
use shared::{Lang, Theme};

use components::episode::EpisodeModal;
use components::layout::{ImportBanner, Sidebar, Toasts};
use components::player_ui::{NowPlaying, PlayerBar};
use pages::{
    discover::DiscoverPage, downloads::DownloadsPage, home::HomePage, library::LibraryPage, podcast::PodcastPage,
    queue::QueuePage, settings::SettingsPage,
};
use player::Player;
use state::{AppState, View};

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    let app = AppState::new();
    provide_context(app);
    let player = Player::new(app);
    provide_context(player);
    app.init(player);
    player::install_link_handler();

    // Thème et langue appliqués au document.
    Effect::new(move |_| {
        let (theme, lang) = app.settings.with(|s| (s.theme, s.language));
        if let Some(root) = document().document_element() {
            let _ = root.set_attribute("data-theme", if theme == Theme::Light { "light" } else { "dark" });
            let _ = root.set_attribute("lang", if lang == Lang::En { "en" } else { "fr" });
        }
    });

    // Raccourcis clavier globaux.
    let _ = window_event_listener(leptos::ev::keydown, move |ev| {
        let in_field = ev
            .target()
            .and_then(|t| wasm_bindgen::JsCast::dyn_into::<web_sys::Element>(t).ok())
            .map(|el| matches!(el.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT"))
            .unwrap_or(false);
        match ev.key().as_str() {
            // En plein écran, Échap est géré par le navigateur pour en sortir.
            "Escape" if player::is_fullscreen() => {}
            "Escape" => {
                if app.modal_episode.get_untracked().is_some() {
                    app.modal_episode.set(None);
                } else if app.now_playing_open.get_untracked() {
                    app.now_playing_open.set(false);
                }
            }
            "f" | "F" if ev.ctrl_key() => {
                ev.prevent_default();
                app.go_root(View::Discover);
            }
            _ if in_field => {}
            " " => {
                ev.prevent_default();
                player.toggle();
            }
            "f" | "F" if player.has_video.get_untracked() => player.toggle_fullscreen(),
            "ArrowLeft" => player.skip_back(),
            "ArrowRight" => player.skip_forward(),
            _ => {}
        }
    });

    let has_player = move || player.current.with(Option::is_some);

    view! {
        <div class="app" class:with-player=has_player>
            <div class="ambient"></div>
            <Sidebar />
            <main class="main" id="main-scroll">
                {move || match app.view.get() {
                    View::Home => view! { <HomePage /> }.into_any(),
                    View::Library => view! { <LibraryPage /> }.into_any(),
                    View::Discover => view! { <DiscoverPage /> }.into_any(),
                    View::Downloads => view! { <DownloadsPage /> }.into_any(),
                    View::Queue => view! { <QueuePage /> }.into_any(),
                    View::Settings => view! { <SettingsPage /> }.into_any(),
                    View::Podcast(id) => view! { <PodcastPage id=id /> }.into_any(),
                    View::Preview(url) => view! { <PodcastPage feed_url=url /> }.into_any(),
                }}
            </main>
            <PlayerBar />
            <NowPlaying />
            <EpisodeModal />
            <ImportBanner />
            <Toasts />
        </div>
    }
}
