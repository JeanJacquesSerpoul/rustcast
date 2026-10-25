//! Récupération et analyse des flux RSS de podcasts.

use anyhow::{anyhow, Context};
use chrono::DateTime;
use shared::{Episode, Podcast};

pub const USER_AGENT: &str = concat!("RustCast/", env!("CARGO_PKG_VERSION"));

pub fn short_hash(s: &str) -> String {
    sha1_smol::Sha1::from(s).digest().to_string()[..16].to_string()
}

pub fn podcast_id(feed_url: &str) -> String {
    short_hash(feed_url.trim())
}

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

pub async fn fetch(client: &reqwest::Client, feed_url: &str) -> anyhow::Result<(Podcast, Vec<Episode>)> {
    let url = normalize_url(feed_url);
    let bytes = client
        .get(&url)
        .send()
        .await
        .context("connexion impossible")?
        .error_for_status()?
        .bytes()
        .await?;
    parse(&url, &bytes)
}

fn normalize_url(url: &str) -> String {
    let url = url.trim();
    if let Some(rest) = url.strip_prefix("feed://") {
        format!("https://{rest}")
    } else if !url.contains("://") {
        format!("https://{url}")
    } else {
        url.to_string()
    }
}

pub fn parse(feed_url: &str, bytes: &[u8]) -> anyhow::Result<(Podcast, Vec<Episode>)> {
    let channel = rss::Channel::read_from(bytes).map_err(|e| anyhow!("flux RSS invalide : {e}"))?;
    let itunes = channel.itunes_ext();
    let id = podcast_id(feed_url);

    let artwork = itunes
        .and_then(|i| i.image())
        .map(str::to_string)
        .or_else(|| channel.image().map(|i| i.url().to_string()));

    let mut categories: Vec<String> = itunes
        .map(|i| i.categories().iter().map(|c| c.text().to_string()).collect())
        .unwrap_or_default();
    if categories.is_empty() {
        categories = channel.categories().iter().map(|c| c.name().to_string()).collect();
    }
    categories.dedup();

    let raw_desc = if channel.description().trim().is_empty() {
        itunes.and_then(|i| i.summary()).unwrap_or_default()
    } else {
        channel.description()
    };

    let podcast = Podcast {
        id: id.clone(),
        feed_url: feed_url.to_string(),
        title: decode_entities(channel.title().trim()),
        author: itunes
            .and_then(|i| i.author())
            .map(str::to_string)
            .or_else(|| channel.managing_editor().map(str::to_string))
            .unwrap_or_default(),
        description: sanitize(raw_desc),
        artwork,
        link: Some(channel.link().to_string()).filter(|l| !l.is_empty()),
        categories,
        last_refreshed: now(),
        added_at: now(),
    };

    let episodes = channel
        .items()
        .iter()
        .filter_map(|item| {
            let enclosure = item.enclosure()?;
            let audio_url = enclosure.url().trim().to_string();
            if audio_url.is_empty() {
                return None;
            }
            let ix = item.itunes_ext();
            let guid = item
                .guid()
                .map(|g| g.value().to_string())
                .unwrap_or_else(|| audio_url.clone());
            let html = item
                .content()
                .filter(|c| !c.trim().is_empty())
                .or(item.description())
                .or_else(|| ix.and_then(|i| i.summary()))
                .unwrap_or_default();
            Some(Episode {
                id: short_hash(&format!("{id}:{guid}")),
                podcast_id: id.clone(),
                guid,
                title: decode_entities(item.title().unwrap_or("Sans titre").trim()),
                summary: truncate(&strip_tags(html), 320),
                description_html: sanitize(html),
                pub_date: item.pub_date().map(parse_date).unwrap_or(0),
                duration: ix.and_then(|i| i.duration()).and_then(parse_duration),
                audio_url,
                mime: Some(enclosure.mime_type().to_string()).filter(|m| !m.is_empty()),
                size: enclosure.length().trim().parse().ok().filter(|&n: &u64| n > 0),
                image: ix.and_then(|i| i.image()).map(str::to_string),
                season: ix.and_then(|i| i.season()).and_then(|s| s.trim().parse().ok()),
                number: ix.and_then(|i| i.episode()).and_then(|s| s.trim().parse().ok()),
                ..Default::default()
            })
        })
        .collect();

    Ok((podcast, episodes))
}

fn parse_date(s: &str) -> i64 {
    let s = s.trim();
    if let Ok(d) = DateTime::parse_from_rfc2822(s) {
        return d.timestamp();
    }
    if let Ok(d) = DateTime::parse_from_rfc3339(s) {
        return d.timestamp();
    }
    // Certains flux utilisent des fuseaux textuels non standard (« CEST », « UTC+2 »...).
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() >= 5 {
        let base = parts[..5].join(" ");
        for fmt in ["%a, %d %b %Y %H:%M:%S", "%d %b %Y %H:%M:%S %z"] {
            if let Ok(d) = chrono::NaiveDateTime::parse_from_str(&base, fmt) {
                return d.and_utc().timestamp();
            }
        }
        if let Ok(d) = DateTime::parse_from_rfc2822(&format!("{base} +0000")) {
            return d.timestamp();
        }
    }
    0
}

fn parse_duration(s: &str) -> Option<u32> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let mut total: u32 = 0;
    for part in s.split(':') {
        let v: f64 = part.trim().parse().ok()?;
        total = total.checked_mul(60)?.checked_add(v as u32)?;
    }
    Some(total).filter(|&d| d > 0)
}

pub fn sanitize(html: &str) -> String {
    let html = html.trim();
    if !html.contains('<') {
        let escaped = decode_entities(html)
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        return linkify(&escaped).replace('\n', "<br>");
    }
    ammonia::Builder::default()
        .link_rel(Some("noopener noreferrer"))
        .rm_tags(["img"])
        .clean(html)
        .to_string()
}

fn linkify(text: &str) -> String {
    text.split(' ')
        .map(|word| {
            if word.starts_with("http://") || word.starts_with("https://") {
                format!(r#"<a href="{0}" rel="noopener noreferrer">{0}</a>"#, word)
            } else {
                word.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => {
                in_tag = true;
                out.push(' ');
            }
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    let decoded = decode_entities(&out);
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        rest = &rest[pos..];
        let end = rest[..rest.len().min(12)].find(';');
        let decoded = end.and_then(|end| {
            let ent = &rest[1..end];
            let ch = match ent {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                "hellip" => Some('…'),
                "rsquo" | "lsquo" => Some('’'),
                "ldquo" | "rdquo" => Some('"'),
                "mdash" => Some('—'),
                "ndash" => Some('–'),
                "eacute" => Some('é'),
                "egrave" => Some('è'),
                "agrave" => Some('à'),
                "ccedil" => Some('ç'),
                _ if ent.starts_with("#x") || ent.starts_with("#X") => {
                    u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32)
                }
                _ if ent.starts_with('#') => ent[1..].parse().ok().and_then(char::from_u32),
                _ => None,
            };
            ch.map(|c| (c, end))
        });
        match decoded {
            Some((c, end)) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max).collect();
    let cut = cut.rsplit_once(' ').map(|(a, _)| a).unwrap_or(&cut);
    format!("{cut}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(parse_duration("1:02:03"), Some(3723));
        assert_eq!(parse_duration("45:10"), Some(2710));
        assert_eq!(parse_duration("3600"), Some(3600));
        assert_eq!(parse_duration(""), None);
    }

    #[test]
    fn dates() {
        assert_eq!(parse_date("Tue, 10 Jun 2003 04:00:00 GMT"), 1055217600);
        assert!(parse_date("Mon, 03 Mar 2025 06:00:00 CEST") > 0);
    }

    #[test]
    fn entities_and_tags() {
        assert_eq!(strip_tags("<p>Hello&nbsp;<b>world</b> &amp; co</p>"), "Hello world & co");
        assert_eq!(decode_entities("l&#8217;été"), "l’été");
    }

    #[test]
    fn parses_minimal_feed() {
        let xml = br#"<?xml version="1.0"?>
<rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd">
<channel><title>Test</title><link>https://ex.com</link><description>Desc</description>
<itunes:author>Moi</itunes:author><itunes:image href="https://ex.com/a.jpg"/>
<item><title>Ep 1</title><guid>g1</guid><pubDate>Tue, 10 Jun 2003 04:00:00 GMT</pubDate>
<enclosure url="https://ex.com/1.mp3" length="1234" type="audio/mpeg"/>
<itunes:duration>10:00</itunes:duration></item>
<item><title>No audio</title></item>
</channel></rss>"#;
        let (p, eps) = parse("https://ex.com/feed", xml).unwrap();
        assert_eq!(p.title, "Test");
        assert_eq!(p.author, "Moi");
        assert_eq!(p.artwork.as_deref(), Some("https://ex.com/a.jpg"));
        assert_eq!(eps.len(), 1);
        assert_eq!(eps[0].duration, Some(600));
        assert_eq!(eps[0].size, Some(1234));
    }
}

#[cfg(test)]
mod network_tests {
    /// Test réseau : `cargo test -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn parses_real_video_feed() {
        let client = reqwest::Client::builder().user_agent(super::USER_AGENT).build().unwrap();
        let (p, eps) = super::fetch(&client, "http://feeds.feedburner.com/TedtalksHD").await.unwrap();
        let videos = eps.iter().filter(|e| e.is_video()).count();
        println!("{} — {} épisodes dont {} vidéos ; 1er : {} ({:?})", p.title, eps.len(), videos, eps[0].audio_url, eps[0].mime);
        assert!(videos > 0 && videos * 2 > eps.len());
    }
}
