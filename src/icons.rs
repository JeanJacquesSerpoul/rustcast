//! Jeu d'icônes vectorielles au trait (24×24).

use leptos::prelude::*;

fn paths(name: &str) -> &'static str {
    match name {
        "play" => r#"<path d="M7 4.6v14.8a1 1 0 0 0 1.5.86l12.3-7.4a1 1 0 0 0 0-1.72L8.5 3.74A1 1 0 0 0 7 4.6z" fill="currentColor" stroke="none"/>"#,
        "pause" => r#"<rect x="6" y="4" width="4.2" height="16" rx="1.3" fill="currentColor" stroke="none"/><rect x="13.8" y="4" width="4.2" height="16" rx="1.3" fill="currentColor" stroke="none"/>"#,
        "home" => r#"<path d="M3 10.5 12 3l9 7.5"/><path d="M5 9.5V20a1 1 0 0 0 1 1h4v-6h4v6h4a1 1 0 0 0 1-1V9.5"/>"#,
        "library" => r#"<rect x="3" y="3" width="7.5" height="7.5" rx="2"/><rect x="13.5" y="3" width="7.5" height="7.5" rx="2"/><rect x="3" y="13.5" width="7.5" height="7.5" rx="2"/><rect x="13.5" y="13.5" width="7.5" height="7.5" rx="2"/>"#,
        "compass" => r#"<circle cx="12" cy="12" r="9"/><path d="m15.5 8.5-2.1 4.9-4.9 2.1 2.1-4.9z" fill="currentColor"/>"#,
        "download" => r#"<path d="M12 4v11"/><path d="m7 10 5 5 5-5"/><path d="M5 20h14"/>"#,
        "queue" => r#"<path d="M4 6h16"/><path d="M4 12h9"/><path d="M4 18h7"/><path d="M16 14.5v7l5.5-3.5z" fill="currentColor"/>"#,
        "settings" => r#"<path d="M4 6h9"/><path d="M19 6h1"/><circle cx="16" cy="6" r="2.5"/><path d="M4 12h3"/><path d="M13 12h7"/><circle cx="10" cy="12" r="2.5"/><path d="M4 18h11"/><circle cx="18" cy="18" r="2.5"/>"#,
        "search" => r#"<circle cx="11" cy="11" r="7"/><path d="m20 20-3.6-3.6"/>"#,
        "back" => r#"<path d="m15 18-6-6 6-6"/>"#,
        "chevron-down" => r#"<path d="m6 9 6 6 6-6"/>"#,
        "chevron-up" => r#"<path d="m6 15 6-6 6 6"/>"#,
        "next" => r#"<path d="M5 5.5v13a1 1 0 0 0 1.5.86l10-6.5a1 1 0 0 0 0-1.72l-10-6.5A1 1 0 0 0 5 5.5z" fill="currentColor" stroke="none"/><path d="M19.5 5v14"/>"#,
        "plus" => r#"<path d="M12 5v14M5 12h14"/>"#,
        "check" => r#"<path d="m5 12.5 4.5 4.5L19 7"/>"#,
        "x" => r#"<path d="M6 6l12 12M18 6 6 18"/>"#,
        "trash" => r#"<path d="M4 7h16"/><path d="M10 11v6M14 11v6"/><path d="M6 7l1 13a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1l1-13"/><path d="M9 7V4h6v3"/>"#,
        "refresh" => r#"<path d="M20 11a8 8 0 0 0-14.9-3.5L4 9"/><path d="M4 4v5h5"/><path d="M4 13a8 8 0 0 0 14.9 3.5L20 15"/><path d="M20 20v-5h-5"/>"#,
        "more" => r#"<circle cx="5" cy="12" r="1.7" fill="currentColor"/><circle cx="12" cy="12" r="1.7" fill="currentColor"/><circle cx="19" cy="12" r="1.7" fill="currentColor"/>"#,
        "volume" => r#"<path d="M4 9v6h4l5 4V5L8 9z" fill="currentColor"/><path d="M16.5 8.5a5 5 0 0 1 0 7"/><path d="M19 6a8.5 8.5 0 0 1 0 12"/>"#,
        "volume-x" => r#"<path d="M4 9v6h4l5 4V5L8 9z" fill="currentColor"/><path d="m17 9.5 5 5M22 9.5l-5 5"/>"#,
        "globe" => r#"<circle cx="12" cy="12" r="9"/><path d="M3 12h18"/><path d="M12 3a14 14 0 0 1 0 18a14 14 0 0 1 0-18"/>"#,
        "moon" => r#"<path d="M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z"/>"#,
        "sun" => r#"<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>"#,
        "headphones" => r#"<path d="M4 15v-3a8 8 0 0 1 16 0v3"/><rect x="3" y="14" width="5" height="7" rx="2"/><rect x="16" y="14" width="5" height="7" rx="2"/>"#,
        "external" => r#"<path d="M14 4h6v6"/><path d="M20 4 11 13"/><path d="M19 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1h5"/>"#,
        "upload" => r#"<path d="M12 15V4"/><path d="m7 9 5-5 5 5"/><path d="M5 20h14"/>"#,
        "folder" => r#"<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>"#,
        "rss" => r#"<path d="M5 11a8 8 0 0 1 8 8"/><path d="M5 5a14 14 0 0 1 14 14"/><circle cx="6" cy="18" r="1.6" fill="currentColor"/>"#,
        "list-plus" => r#"<path d="M4 6h12M4 12h8M4 18h7"/><path d="M18 13v8M14 17h8"/>"#,
        "play-next" => r#"<path d="M4 12h11M4 18h11"/><path d="M4 6h6"/><path d="M15 3.5v5l4-2.5z" fill="currentColor"/>"#,
        "check-circle" => r#"<circle cx="12" cy="12" r="9"/><path d="m8 12.5 3 3 5-6"/>"#,
        "circle" => r#"<circle cx="12" cy="12" r="9"/>"#,
        "notes" => r#"<path d="M6 3h9l4 4v14H6z"/><path d="M14 3v5h5"/><path d="M9 13h7M9 17h5"/>"#,
        "gauge" => r#"<path d="M12 14l4-4"/><path d="M3.3 17a9 9 0 1 1 17.4 0"/>"#,
        "arrow-up" => r#"<path d="M12 19V5"/><path d="m6 11 6-6 6 6"/>"#,
        "arrow-down" => r#"<path d="M12 5v14"/><path d="m6 13 6 6 6-6"/>"#,
        "sparkles" => r#"<path d="M12 3l1.8 4.7L18.5 9.5l-4.7 1.8L12 16l-1.8-4.7L5.5 9.5l4.7-1.8z" fill="currentColor" stroke="none"/><path d="M19 15l.8 2.2L22 18l-2.2.8L19 21l-.8-2.2L16 18l2.2-.8z" fill="currentColor" stroke="none"/>"#,
        "maximize" => r#"<path d="M15 3h6v6"/><path d="M9 21H3v-6"/><path d="M21 3l-7 7"/><path d="M3 21l7-7"/>"#,
        "sort" => r#"<path d="M7 4v16"/><path d="m3 8 4-4 4 4"/><path d="M17 20V4"/><path d="m13 16 4 4 4-4"/>"#,
        "video" => r#"<rect x="2.5" y="6" width="13" height="12" rx="2.5"/><path d="m15.5 10.5 6-3.5v10l-6-3.5z"/>"#,
        "info" => r#"<circle cx="12" cy="12" r="9"/><path d="M12 11v6"/><circle cx="12" cy="7.8" r="1" fill="currentColor"/>"#,
        _ => r#"<circle cx="12" cy="12" r="9"/>"#,
    }
}

#[component]
pub fn Icon(name: &'static str, #[prop(default = 20)] size: u32) -> impl IntoView {
    view! {
        <svg
            class="icon"
            width=size
            height=size
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
            inner_html=paths(name)
        ></svg>
    }
}

/// Icône de saut avec le nombre de secondes affiché au centre.
#[component]
pub fn SkipIcon(#[prop(into)] seconds: Signal<u32>, forward: bool, #[prop(default = 28)] size: u32) -> impl IntoView {
    let arc = if forward {
        r#"<path d="M20.5 12a8.5 8.5 0 1 1-2.6-6.1"/><path d="M20.5 3.5v4.8h-4.8"/>"#
    } else {
        r#"<path d="M3.5 12a8.5 8.5 0 1 0 2.6-6.1"/><path d="M3.5 3.5v4.8h4.8"/>"#
    };
    view! {
        <svg
            class="icon"
            width=size
            height=size
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
            stroke-linecap="round"
            stroke-linejoin="round"
        >
            <g inner_html=arc></g>
            <text
                x="12"
                y="15.6"
                text-anchor="middle"
                font-size="7.6"
                font-weight="700"
                fill="currentColor"
                stroke="none"
                font-family="inherit"
            >
                {move || seconds.get().to_string()}
            </text>
        </svg>
    }
}
