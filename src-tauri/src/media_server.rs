//! Serveur HTTP local qui sert les épisodes téléchargés au lecteur, sous Linux.
//!
//! WebKitGTK ne transmet pas les schémas personnalisés comme `asset://` à GStreamer :
//! la lecture d'un fichier local y échoue aussitôt (« FormatError »), alors que le même
//! fichier se lit sous Windows. On sert donc les téléchargements sur 127.0.0.1, derrière
//! un jeton aléatoire, avec les mêmes autorisations que le protocole `asset` et la gestion
//! des plages (Range) dont GStreamer a besoin pour se déplacer dans le fichier.

use std::hash::{BuildHasher, Hasher};
use std::io::SeekFrom;
use std::path::Path;

use tauri::path::SafePathBuf;
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

/// Variable globale par laquelle l'interface découvre l'adresse du serveur.
const JS_GLOBAL: &str = "__RUSTCAST_MEDIA__";
/// Taille maximale acceptée pour les en-têtes d'une requête.
const MAX_HEAD: usize = 16 * 1024;

pub struct MediaServer {
    listener: std::net::TcpListener,
    token: String,
}

impl MediaServer {
    /// Réserve un port local ; le serveur ne répond qu'une fois [`Self::spawn`] appelé.
    pub fn bind() -> std::io::Result<Self> {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        Ok(Self { listener, token: random_token() })
    }

    /// Script injecté dans la page : préfixe des adresses, suivi du chemin encodé.
    pub fn init_script(&self) -> String {
        let port = self.listener.local_addr().map(|a| a.port()).unwrap_or_default();
        format!("window.{JS_GLOBAL} = \"http://127.0.0.1:{port}/{}/\";", self.token)
    }

    pub fn spawn(self, app: AppHandle) {
        tauri::async_runtime::spawn(async move {
            let Ok(listener) = tokio::net::TcpListener::from_std(self.listener) else {
                return;
            };
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    continue;
                };
                let app = app.clone();
                let token = self.token.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = serve(stream, &app, &token).await;
                });
            }
        });
    }
}

/// 128 bits imprévisibles, tirés des clés aléatoires de la bibliothèque standard.
fn random_token() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    (0..2)
        .map(|_| {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u128(nanos);
            format!("{:016x}", h.finish())
        })
        .collect()
}

async fn serve(stream: TcpStream, app: &AppHandle, token: &str) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line).await?;
    let mut range = None;
    let mut head_len = request_line.len();
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).await?;
        head_len += n;
        if n == 0 || line.trim().is_empty() || head_len > MAX_HEAD {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("range") {
                range = Some(value.trim().to_string());
            }
        }
    }
    let mut stream = reader.into_inner();

    let mut parts = request_line.split_whitespace();
    let (method, target) = (parts.next().unwrap_or_default(), parts.next().unwrap_or_default());
    if method != "GET" && method != "HEAD" {
        return respond(&mut stream, "405 Method Not Allowed", &[]).await;
    }
    let target = target.split(['?', '#']).next().unwrap_or_default();
    let Some(encoded) = target.strip_prefix('/').and_then(|t| t.strip_prefix(token)).and_then(|t| t.strip_prefix('/')) else {
        return respond(&mut stream, "404 Not Found", &[]).await;
    };
    let Ok(path) = urlencoding::decode(encoded) else {
        return respond(&mut stream, "400 Bad Request", &[]).await;
    };
    // Mêmes règles que le protocole `asset` : chemin absolu sans « .. », dans la portée autorisée.
    let Ok(path) = SafePathBuf::new(path.as_ref().into()) else {
        return respond(&mut stream, "403 Forbidden", &[]).await;
    };
    if !app.asset_protocol_scope().is_allowed(&path) {
        return respond(&mut stream, "403 Forbidden", &[]).await;
    }
    let Ok(mut file) = tokio::fs::File::open(&path).await else {
        return respond(&mut stream, "404 Not Found", &[]).await;
    };
    let size = file.metadata().await?.len();
    let content_type = mime_type(path.as_ref());

    let (status, start, len) = match range.as_deref().map(|r| parse_range(r, size)) {
        None => ("200 OK", 0, size),
        Some(Some((start, end))) => ("206 Partial Content", start, end - start + 1),
        Some(None) => {
            let unsatisfiable = format!("bytes */{size}");
            return respond(&mut stream, "416 Range Not Satisfiable", &[("Content-Range", &unsatisfiable)]).await;
        }
    };
    let length = len.to_string();
    let content_range = format!("bytes {start}-{}/{size}", (start + len).saturating_sub(1));
    let mut headers = vec![("Content-Type", content_type), ("Content-Length", length.as_str())];
    if status.starts_with("206") {
        headers.push(("Content-Range", &content_range));
    }
    respond(&mut stream, status, &headers).await?;
    if method == "GET" {
        file.seek(SeekFrom::Start(start)).await?;
        tokio::io::copy(&mut file.take(len), &mut stream).await?;
    }
    stream.shutdown().await
}

async fn respond(stream: &mut TcpStream, status: &str, headers: &[(&str, &str)]) -> std::io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {status}\r\nAccept-Ranges: bytes\r\nCache-Control: no-store\r\nConnection: close\r\n"
    );
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    if !headers.iter().any(|(name, _)| *name == "Content-Length") {
        head.push_str("Content-Length: 0\r\n");
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes()).await
}

/// Plage d'octets demandée (`bytes=début-fin`, `bytes=début-` ou `bytes=-suffixe`), bornes incluses.
fn parse_range(header: &str, size: u64) -> Option<(u64, u64)> {
    let spec = header.strip_prefix("bytes=")?.split(',').next()?.trim();
    let (start, end) = spec.split_once('-')?;
    let last = size.checked_sub(1)?;
    let (start, end) = match (start.trim(), end.trim()) {
        ("", suffix) => {
            let n: u64 = suffix.parse().ok()?;
            (size.saturating_sub(n), last)
        }
        (start, "") => (start.parse().ok()?, last),
        (start, end) => (start.parse().ok()?, end.parse::<u64>().ok()?.min(last)),
    };
    (start <= end).then_some((start, end))
}

fn mime_type(path: &Path) -> &'static str {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    match ext.as_str() {
        "mp3" => "audio/mpeg",
        "m4a" | "aac" => "audio/mp4",
        "ogg" | "oga" | "opus" => "audio/ogg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::parse_range;

    #[test]
    fn ranges() {
        assert_eq!(parse_range("bytes=0-15", 100), Some((0, 15)));
        assert_eq!(parse_range("bytes=10-", 100), Some((10, 99)));
        assert_eq!(parse_range("bytes=-30", 100), Some((70, 99)));
        assert_eq!(parse_range("bytes=90-500", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=100-", 100), None);
        assert_eq!(parse_range("bytes=0-", 0), None);
        assert_eq!(parse_range("items=0-1", 100), None);
    }
}
