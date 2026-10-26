use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::{Lang, Theme};

use crate::api;
use crate::components::common::PageHeader;
use crate::icons::Icon;
use crate::player::player;
use crate::state::{app, async_data, ToastKind};
use crate::util::{fmt_bytes_lang, release_url, REPO_URL, VERSION};

const SKIPS: [u32; 6] = [5, 10, 15, 30, 45, 60];

#[component]
fn Row(icon: &'static str, title: &'static str, desc: &'static str, children: Children) -> impl IntoView {
    let app = app();
    view! {
        <div class="setting-row">
            <div class="setting-icon"><Icon name=icon size=18 /></div>
            <div class="setting-text">
                <div class="setting-title">{move || app.tr(title)}</div>
                <div class="setting-desc">{move || app.tr(desc)}</div>
            </div>
            <div class="setting-control">{children()}</div>
        </div>
    }
}

#[component]
pub fn SettingsPage() -> impl IntoView {
    let app = app();
    let storage = async_data(move || {
        app.library_rev.track();
        api::storage_info()
    });
    let s = move || app.settings.get();
    let player = player();
    let confirm_reset = RwSignal::new(false);
    let resetting = RwSignal::new(false);
    let reset = move |_| {
        resetting.set(true);
        player.close();
        spawn_local(async move {
            match api::reset_app().await {
                Ok(()) => {
                    if let Ok(s) = api::get_settings().await {
                        app.settings.set(s);
                    }
                    app.reload_library().await;
                    app.toast(app.tr("toast.reset"), ToastKind::Success);
                }
                Err(e) => app.error(e),
            }
            resetting.set(false);
            confirm_reset.set(false);
        });
    };

    let skip_select = move |back: bool| {
        view! {
            <select
                class="select"
                on:change=move |ev| {
                    let v: u32 = event_target_value(&ev).parse().unwrap_or(15);
                    app.update_settings(|s| if back { s.skip_back = v } else { s.skip_forward = v });
                }
            >
                {SKIPS.iter().map(|&v| view! {
                    <option
                        value=v
                        selected=move || { let s = s(); (if back { s.skip_back } else { s.skip_forward }) == v }
                    >
                        {format!("{v} s")}
                    </option>
                }).collect_view()}
            </select>
        }
    };

    view! {
        <div class="page settings">
            <PageHeader title=Signal::derive(move || app.tr("nav.settings").to_string()) />

            <section class="settings-group">
                <h2>{move || app.tr("settings.appearance")}</h2>
                <Row icon="globe" title="settings.language" desc="settings.language_desc">
                    <div class="segmented">
                        <button class:active=move || s().language == Lang::Fr on:click=move |_| app.update_settings(|s| s.language = Lang::Fr)>
                            "Français"
                        </button>
                        <button class:active=move || s().language == Lang::En on:click=move |_| app.update_settings(|s| s.language = Lang::En)>
                            "English"
                        </button>
                    </div>
                </Row>
                <Row icon="moon" title="settings.theme" desc="settings.theme_desc">
                    <div class="segmented">
                        <button class:active=move || s().theme == Theme::Dark on:click=move |_| app.update_settings(|s| s.theme = Theme::Dark)>
                            <Icon name="moon" size=14 />
                            {move || app.tr("settings.dark")}
                        </button>
                        <button class:active=move || s().theme == Theme::Light on:click=move |_| app.update_settings(|s| s.theme = Theme::Light)>
                            <Icon name="sun" size=14 />
                            {move || app.tr("settings.light")}
                        </button>
                    </div>
                </Row>
            </section>

            <section class="settings-group">
                <h2>{move || app.tr("settings.playback")}</h2>
                <Row icon="back" title="settings.skip_back" desc="settings.skip_back_desc">
                    {skip_select(true)}
                </Row>
                <Row icon="next" title="settings.skip_forward" desc="settings.skip_forward_desc">
                    {skip_select(false)}
                </Row>
                <Row icon="trash" title="settings.auto_delete" desc="settings.auto_delete_desc">
                    <label class="switch">
                        <input
                            type="checkbox"
                            prop:checked=move || s().auto_delete_played
                            on:change=move |ev| {
                                let v = event_target_checked(&ev);
                                app.update_settings(|s| s.auto_delete_played = v);
                            }
                        />
                        <span></span>
                    </label>
                </Row>
            </section>

            <section class="settings-group">
                <h2>{move || app.tr("settings.library")}</h2>
                <Row icon="upload" title="settings.import" desc="settings.import_desc">
                    <button
                        class="btn ghost"
                        class:loading=move || app.importing.with(Option::is_some)
                        on:click=move |_| app.import_opml()
                    >
                        {move || app.tr("settings.import_btn")}
                    </button>
                </Row>
                <Row icon="download" title="settings.export" desc="settings.export_desc">
                    <button class="btn ghost" on:click=move |_| app.export_opml()>
                        {move || app.tr("settings.export_btn")}
                    </button>
                </Row>
                <Row icon="folder" title="settings.storage" desc="settings.storage_desc">
                    <span class="muted">
                        {move || storage.get().and_then(Result::ok).map(|s| fmt_bytes_lang(app.lang(), s.downloads_bytes))}
                    </span>
                    <button class="btn ghost" on:click=move |_| spawn_local(async move { let _ = api::open_data_dir().await; })>
                        {move || app.tr("dl.open_folder")}
                    </button>
                </Row>
            </section>

            <section class="settings-group">
                <h2>{move || app.tr("settings.reset_group")}</h2>
                <Row icon="refresh" title="settings.reset" desc="settings.reset_desc">
                    <Show
                        when=move || confirm_reset.get()
                        fallback=move || view! {
                            <button class="btn ghost danger" on:click=move |_| confirm_reset.set(true)>
                                {move || app.tr("settings.reset_btn")}
                            </button>
                        }
                    >
                        <button class="btn ghost" on:click=move |_| confirm_reset.set(false)>
                            {move || app.tr("settings.reset_cancel")}
                        </button>
                        <button class="btn danger" class:loading=move || resetting.get() on:click=reset>
                            <Icon name="trash" size=16 />
                            {move || app.tr("settings.reset_confirm")}
                        </button>
                    </Show>
                </Row>
            </section>

            <section class="settings-group">
                <h2>{move || app.tr("settings.shortcuts")}</h2>
                <div class="shortcuts">
                    <div><kbd>{move || app.tr("kbd.space")}</kbd><span>{move || app.tr("shortcut.play")}</span></div>
                    <div><kbd>"←"</kbd><kbd>"→"</kbd><span>{move || app.tr("shortcut.skip")}</span></div>
                    <div><kbd>"Ctrl"</kbd><kbd>"F"</kbd><span>{move || app.tr("shortcut.search")}</span></div>
                    <div><kbd>"F"</kbd><span>{move || app.tr("shortcut.fullscreen")}</span></div>
                    <div><kbd>{move || app.tr("kbd.esc")}</kbd><span>{move || app.tr("shortcut.close")}</span></div>
                </div>
            </section>

            <section class="settings-group" id="about">
                <h2>{move || app.tr("about.title")}</h2>
                <div class="about">
                    <img src="public/logo.svg" alt="" />
                    <div class="about-text">
                        <div class="about-name">
                            <strong>"RustCast"</strong>
                            <a class="version-pill" href=release_url() title=move || app.tr("about.release")>
                                {format!("v{VERSION}")}
                            </a>
                        </div>
                        <p>{move || app.tr("settings.about")}</p>
                        <div class="about-links">
                            <a href=REPO_URL>
                                <Icon name="external" size=14 />
                                {move || app.tr("about.source")}
                            </a>
                            <a href=release_url()>
                                <Icon name="sparkles" size=14 />
                                {move || app.tr("about.release")}
                            </a>
                            <a href=format!("{REPO_URL}/issues")>
                                <Icon name="info" size=14 />
                                {move || app.tr("about.issues")}
                            </a>
                            <a href=format!("{REPO_URL}/blob/main/LICENSE")>
                                <Icon name="notes" size=14 />
                                {move || app.tr("about.license")}
                            </a>
                        </div>
                    </div>
                </div>
            </section>
        </div>
    }
}
