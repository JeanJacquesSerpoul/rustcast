//! Persistance de la bibliothèque dans un fichier JSON du dossier de données.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use shared::{Episode, EpisodeView, Podcast, PodcastSummary, Settings};

#[derive(Default, Serialize, Deserialize)]
pub struct Library {
    pub podcasts: Vec<Podcast>,
    /// Épisodes par identifiant de podcast, triés du plus récent au plus ancien.
    pub episodes: HashMap<String, Vec<Episode>>,
    pub queue: Vec<String>,
    pub settings: Settings,
}

impl Library {
    pub fn load(path: &Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec(self)?)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn podcast(&self, id: &str) -> Option<&Podcast> {
        self.podcasts.iter().find(|p| p.id == id)
    }

    pub fn episode(&self, id: &str) -> Option<&Episode> {
        self.episodes.values().flatten().find(|e| e.id == id)
    }

    pub fn episode_mut(&mut self, id: &str) -> Option<&mut Episode> {
        self.episodes.values_mut().flatten().find(|e| e.id == id)
    }

    pub fn view(&self, ep: &Episode) -> EpisodeView {
        let podcast = self.podcast(&ep.podcast_id);
        EpisodeView {
            episode: ep.clone(),
            podcast_title: podcast.map(|p| p.title.clone()).unwrap_or_default(),
            podcast_artwork: podcast.and_then(|p| p.artwork.clone()),
        }
    }

    pub fn view_by_id(&self, id: &str) -> Option<EpisodeView> {
        self.episode(id).map(|e| self.view(e))
    }

    pub fn summary(&self, p: &Podcast) -> PodcastSummary {
        let eps = self.episodes.get(&p.id).map(Vec::as_slice).unwrap_or(&[]);
        PodcastSummary {
            podcast: p.clone(),
            episode_count: eps.len(),
            unplayed_count: eps.iter().filter(|e| !e.played).count(),
            downloaded_count: eps.iter().filter(|e| e.local_path.is_some()).count(),
            latest_pub: eps.iter().map(|e| e.pub_date).max().unwrap_or(0),
        }
    }

    /// Fusionne des épisodes fraîchement récupérés en conservant l'état local.
    /// Renvoie le nombre de nouveaux épisodes.
    pub fn merge_episodes(&mut self, podcast_id: &str, fresh: Vec<Episode>) -> usize {
        let old: HashMap<String, Episode> = self
            .episodes
            .remove(podcast_id)
            .unwrap_or_default()
            .into_iter()
            .map(|e| (e.id.clone(), e))
            .collect();
        let had_any = !old.is_empty();
        let mut new_count = 0;
        let mut merged: Vec<Episode> = fresh
            .into_iter()
            .map(|mut e| {
                if let Some(prev) = old.get(&e.id) {
                    e.position = prev.position;
                    e.played = prev.played;
                    e.local_path = prev.local_path.clone();
                    e.local_size = prev.local_size;
                } else if had_any {
                    new_count += 1;
                }
                e
            })
            .collect();
        // Les épisodes téléchargés qui ont disparu du flux sont conservés.
        let fresh_ids: std::collections::HashSet<String> =
            merged.iter().map(|e| e.id.clone()).collect();
        for (id, e) in old {
            if !fresh_ids.contains(&id) && e.local_path.is_some() {
                merged.push(e);
            }
        }
        merged.sort_by(|a, b| b.pub_date.cmp(&a.pub_date));
        self.episodes.insert(podcast_id.to_string(), merged);
        new_count
    }
}

/// Bibliothèque partagée avec un indicateur de modification pour l'enregistrement différé.
pub struct Store {
    pub lib: std::sync::Mutex<Library>,
    pub path: PathBuf,
    pub data_dir: PathBuf,
    dirty: AtomicBool,
}

impl Store {
    pub fn open(data_dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&data_dir);
        let path = data_dir.join("library.json");
        Self {
            lib: std::sync::Mutex::new(Library::load(&path)),
            path,
            data_dir,
            dirty: AtomicBool::new(false),
        }
    }

    pub fn downloads_dir(&self) -> PathBuf {
        self.data_dir.join("downloads")
    }

    pub fn with<R>(&self, f: impl FnOnce(&Library) -> R) -> R {
        f(&self.lib.lock().unwrap())
    }

    /// Modifie la bibliothèque et marque l'état comme à enregistrer.
    pub fn update<R>(&self, f: impl FnOnce(&mut Library) -> R) -> R {
        let r = f(&mut self.lib.lock().unwrap());
        self.dirty.store(true, Ordering::Release);
        r
    }

    pub fn flush(&self) {
        if self.dirty.swap(false, Ordering::AcqRel) {
            let lib = self.lib.lock().unwrap();
            if let Err(e) = lib.save(&self.path) {
                eprintln!("Échec de l'enregistrement de la bibliothèque : {e}");
                self.dirty.store(true, Ordering::Release);
            }
        }
    }
}
