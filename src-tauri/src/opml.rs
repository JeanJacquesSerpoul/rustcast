//! Import et export des abonnements au format OPML.

use quick_xml::events::Event;
use quick_xml::Reader;
use shared::Podcast;

/// Décode le contenu d'un fichier OPML : UTF-8 (avec ou sans BOM), UTF-16 ou Latin-1.
pub fn decode(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    let utf16 = |le: bool| {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) })
            .collect();
        String::from_utf16_lossy(&units)
    };
    match bytes {
        [0xFF, 0xFE, ..] => utf16(true),
        [0xFE, 0xFF, ..] => utf16(false),
        _ => match std::str::from_utf8(bytes) {
            Ok(s) => s.to_string(),
            // Pas de l'UTF-8 valide : on suppose du Latin-1 (fréquent dans les vieux exports).
            Err(_) => bytes.iter().map(|&b| b as char).collect(),
        },
    }
}

pub fn parse_feed_urls(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().check_end_names = false;
    let mut urls = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.name().as_ref().eq_ignore_ascii_case(b"outline") => {
                for attr in e.attributes().with_checks(false).flatten() {
                    if !attr.key.as_ref().eq_ignore_ascii_case(b"xmlurl") {
                        continue;
                    }
                    let value = match attr.unescape_value() {
                        Ok(v) => v.into_owned(),
                        Err(_) => crate::feed::decode_entities(&String::from_utf8_lossy(&attr.value)),
                    };
                    let url = value.trim().to_string();
                    if !url.is_empty() && !urls.contains(&url) {
                        urls.push(url);
                    }
                }
            }
            Ok(Event::Eof) => break,
            // Un fichier mal formé : on garde ce qui a déjà été lu.
            Err(_) => break,
            _ => {}
        }
    }
    urls
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn export(podcasts: &[Podcast]) -> String {
    let date = chrono::Utc::now().to_rfc2822();
    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<opml version=\"2.0\">\n  <head>\n    <title>RustCast</title>\n    <dateCreated>{date}</dateCreated>\n  </head>\n  <body>\n"
    );
    for p in podcasts {
        out.push_str(&format!(
            "    <outline type=\"rss\" text=\"{0}\" title=\"{0}\" xmlUrl=\"{1}\"",
            esc(&p.title),
            esc(&p.feed_url)
        ));
        if let Some(link) = p.link.as_deref().filter(|l| !l.is_empty()) {
            out.push_str(&format!(" htmlUrl=\"{}\"", esc(link)));
        }
        out.push_str("/>\n");
    }
    out.push_str("  </body>\n</opml>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let p = shared::Podcast {
            title: "A & B <\"C\">".into(),
            feed_url: "https://ex.com/f?a=1&b=2".into(),
            link: Some("https://ex.com".into()),
            ..Default::default()
        };
        let xml = export(&[p]);
        assert!(xml.contains("htmlUrl=\"https://ex.com\""));
        assert_eq!(parse_feed_urls(&xml), vec!["https://ex.com/f?a=1&b=2"]);
    }

    #[test]
    fn nested_and_case_insensitive() {
        let xml = r#"<?xml version="1.0"?>
<opml version="1.0"><body>
  <outline text="Tech">
    <outline text="A" xmlUrl="https://a.com/feed"/>
    <outline text="B" XMLURL=" https://b.com/rss "></outline>
  </outline>
  <outline text="A again" xmlUrl="https://a.com/feed"/>
  <outline text="Folder without feed"/>
</body></opml>"#;
        assert_eq!(parse_feed_urls(xml), vec!["https://a.com/feed", "https://b.com/rss"]);
    }

    #[test]
    fn malformed_keeps_what_was_read() {
        let xml = r#"<opml><body><outline xmlUrl="https://a.com/f"/><outline text="x &nbsp; y" xmlUrl="https://b.com/f"></body>"#;
        assert_eq!(parse_feed_urls(xml), vec!["https://a.com/f", "https://b.com/f"]);
    }

    #[test]
    fn decodes_encodings() {
        let utf8_bom = [&[0xEF, 0xBB, 0xBF][..], "<opml>é</opml>".as_bytes()].concat();
        assert_eq!(decode(&utf8_bom), "<opml>é</opml>");
        let latin1 = b"<opml>\xe9</opml>";
        assert_eq!(decode(latin1), "<opml>é</opml>");
        let mut utf16 = vec![0xFF, 0xFE];
        utf16.extend("<a/>".encode_utf16().flat_map(|u| u.to_le_bytes()));
        assert_eq!(decode(&utf16), "<a/>");
    }
}
