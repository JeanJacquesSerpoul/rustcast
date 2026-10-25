//! Types partagés entre le backend Tauri et le frontend Leptos.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Podcast {
    pub id: String,
    pub feed_url: String,
    pub title: String,
    pub author: String,
    /// Description nettoyée (HTML sûr).
    pub description: String,
    pub artwork: Option<String>,
    pub link: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub last_refreshed: i64,
    #[serde(default)]
    pub added_at: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PodcastSummary {
    pub podcast: Podcast,
    pub episode_count: usize,
    pub unplayed_count: usize,
    pub downloaded_count: usize,
    pub latest_pub: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Episode {
    pub id: String,
    pub podcast_id: String,
    pub guid: String,
    pub title: String,
    /// Résumé en texte brut, court.
    pub summary: String,
    /// Notes d'épisode en HTML nettoyé.
    pub description_html: String,
    pub pub_date: i64,
    /// Durée en secondes.
    pub duration: Option<u32>,
    pub audio_url: String,
    pub mime: Option<String>,
    pub size: Option<u64>,
    pub image: Option<String>,
    pub season: Option<u32>,
    pub number: Option<u32>,
    #[serde(default)]
    pub position: f64,
    #[serde(default)]
    pub played: bool,
    #[serde(default)]
    pub local_path: Option<String>,
    #[serde(default)]
    pub local_size: Option<u64>,
}

impl Episode {
    pub fn progress_ratio(&self) -> f64 {
        match self.duration {
            Some(d) if d > 0 => (self.position / d as f64).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }
}

/// Un épisode accompagné des informations de son podcast, pour les listes transverses.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EpisodeView {
    pub episode: Episode,
    pub podcast_title: String,
    pub podcast_artwork: Option<String>,
}

impl EpisodeView {
    pub fn artwork(&self) -> Option<String> {
        self.episode
            .image
            .clone()
            .or_else(|| self.podcast_artwork.clone())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FeedPreview {
    pub podcast: Podcast,
    pub episodes: Vec<EpisodeView>,
    pub subscribed: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    pub collection_id: u64,
    pub title: String,
    pub author: String,
    pub artwork: Option<String>,
    pub feed_url: Option<String>,
    pub genre: Option<String>,
    pub episode_count: Option<u32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lang {
    #[default]
    Fr,
    En,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub language: Lang,
    pub theme: Theme,
    pub skip_back: u32,
    pub skip_forward: u32,
    pub playback_rate: f64,
    pub volume: f64,
    pub region: String,
    pub auto_delete_played: bool,
    pub last_episode: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: Lang::Fr,
            theme: Theme::Dark,
            skip_back: 15,
            skip_forward: 30,
            playback_rate: 1.0,
            volume: 1.0,
            region: "fr".into(),
            auto_delete_played: false,
            last_episode: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DownloadStatus {
    Queued,
    Downloading,
    Done,
    Failed(String),
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub episode_id: String,
    pub podcast_id: String,
    pub title: String,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub status: DownloadStatus,
}

impl DownloadProgress {
    pub fn ratio(&self) -> f64 {
        match self.total {
            Some(t) if t > 0 => (self.downloaded as f64 / t as f64).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StorageInfo {
    pub downloads_bytes: u64,
    pub downloads_count: usize,
    pub data_dir: String,
}

/// Bilan d'un import OPML.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ImportReport {
    /// Faux si l'utilisateur a fermé la boîte de dialogue sans choisir de fichier.
    pub picked: bool,
    pub total: usize,
    pub added: usize,
    pub already: usize,
    /// URL des flux qui n'ont pas pu être chargés.
    pub failed: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImportProgress {
    pub done: usize,
    pub total: usize,
}

/// Événement émis par le backend quand la bibliothèque change.
pub const EVT_LIBRARY: &str = "library-changed";
pub const EVT_DOWNLOAD: &str = "download-progress";
pub const EVT_IMPORT: &str = "import-progress";
