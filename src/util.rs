//! Formatage des durées, dates et tailles selon la langue.

use shared::Lang;

pub fn fmt_clock(secs: f64) -> String {
    if !secs.is_finite() || secs < 0.0 {
        return "0:00".into();
    }
    let s = secs as u64;
    let (h, m, s) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

pub fn fmt_duration(secs: u32) -> String {
    let (h, m) = (secs / 3600, (secs % 3600) / 60);
    match (h, m) {
        (0, 0) => format!("{secs} s"),
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m:02}"),
    }
}

pub fn fmt_remaining(lang: Lang, secs: f64) -> String {
    let d = fmt_duration(secs.max(60.0) as u32);
    match lang {
        Lang::Fr => format!("{d} restantes"),
        Lang::En => format!("{d} left"),
    }
}

const MONTHS_FR: [&str; 12] = [
    "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc.",
];
const MONTHS_EN: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

pub fn fmt_date(lang: Lang, ts: i64) -> String {
    if ts <= 0 {
        return String::new();
    }
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(ts as f64 * 1000.0));
    let now = js_sys::Date::new_0();
    let start_of_day = |d: &js_sys::Date| {
        js_sys::Date::new_with_year_month_day(d.get_full_year(), d.get_month() as i32, d.get_date() as i32)
            .get_time()
    };
    let days = ((start_of_day(&now) - start_of_day(&date)) / 86_400_000.0).round() as i64;
    match (lang, days) {
        (Lang::Fr, 0) => "Aujourd’hui".into(),
        (Lang::En, 0) => "Today".into(),
        (Lang::Fr, 1) => "Hier".into(),
        (Lang::En, 1) => "Yesterday".into(),
        (Lang::Fr, 2..=6) => format!("Il y a {days} jours"),
        (Lang::En, 2..=6) => format!("{days} days ago"),
        _ => {
            let (d, m, y) = (date.get_date(), date.get_month() as usize, date.get_full_year());
            let same_year = y == now.get_full_year();
            match (lang, same_year) {
                (Lang::Fr, true) => format!("{d} {}", MONTHS_FR[m]),
                (Lang::Fr, false) => format!("{d} {} {y}", MONTHS_FR[m]),
                (Lang::En, true) => format!("{} {d}", MONTHS_EN[m]),
                (Lang::En, false) => format!("{} {d}, {y}", MONTHS_EN[m]),
            }
        }
    }
}

pub fn fmt_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["o", "Ko", "Mo", "Go", "To"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{bytes} o")
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}

pub fn fmt_bytes_lang(lang: Lang, bytes: u64) -> String {
    let s = fmt_bytes(bytes);
    match lang {
        Lang::Fr => s.replace('.', ","),
        Lang::En => s.replace("Ko", "KB").replace("Mo", "MB").replace("Go", "GB").replace("To", "TB").replace(" o", " B"),
    }
}
