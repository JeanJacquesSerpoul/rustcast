//! Recherche et classements via les API publiques d'Apple Podcasts.

use serde::Deserialize;
use shared::SearchResult;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItunesItem {
    collection_id: Option<u64>,
    collection_name: Option<String>,
    artist_name: Option<String>,
    artwork_url600: Option<String>,
    artwork_url100: Option<String>,
    feed_url: Option<String>,
    primary_genre_name: Option<String>,
    track_count: Option<u32>,
}

#[derive(Deserialize)]
struct ItunesResponse {
    results: Vec<ItunesItem>,
}

impl ItunesItem {
    fn into_result(self) -> Option<SearchResult> {
        Some(SearchResult {
            collection_id: self.collection_id?,
            title: self.collection_name?,
            author: self.artist_name.unwrap_or_default(),
            artwork: self.artwork_url600.or(self.artwork_url100),
            feed_url: self.feed_url,
            genre: self.primary_genre_name,
            episode_count: self.track_count,
        })
    }
}

pub async fn search(client: &reqwest::Client, term: &str, country: &str) -> anyhow::Result<Vec<SearchResult>> {
    let resp: ItunesResponse = client
        .get("https://itunes.apple.com/search")
        .query(&[("media", "podcast"), ("entity", "podcast"), ("limit", "40"), ("term", term), ("country", country)])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(resp
        .results
        .into_iter()
        .filter_map(ItunesItem::into_result)
        .filter(|r| r.feed_url.is_some())
        .collect())
}

#[derive(Deserialize)]
struct ChartResponse {
    feed: ChartFeed,
}

#[derive(Deserialize)]
struct ChartFeed {
    results: Vec<ChartItem>,
}

#[derive(Deserialize)]
struct ChartItem {
    id: String,
}

/// Classement des podcasts les plus populaires d'un pays, enrichi des URL de flux.
pub async fn top(client: &reqwest::Client, country: &str) -> anyhow::Result<Vec<SearchResult>> {
    let chart: ChartResponse = client
        .get(format!(
            "https://rss.applemarketingtools.com/api/v2/{country}/podcasts/top/48/podcasts.json"
        ))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let ids: Vec<String> = chart.feed.results.into_iter().map(|i| i.id).collect();
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let resp: ItunesResponse = client
        .get("https://itunes.apple.com/lookup")
        .query(&[("id", ids.join(",").as_str()), ("entity", "podcast"), ("country", country)])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let mut results: Vec<SearchResult> = resp
        .results
        .into_iter()
        .filter_map(ItunesItem::into_result)
        .filter(|r| r.feed_url.is_some())
        .collect();
    // L'API de recherche ne garantit pas l'ordre : on rétablit celui du classement.
    results.sort_by_key(|r| {
        ids.iter()
            .position(|id| id == &r.collection_id.to_string())
            .unwrap_or(usize::MAX)
    });
    Ok(results)
}

#[cfg(test)]
mod tests {
    /// Test réseau : `cargo test -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn search_top_and_fetch_feed() {
        let client = reqwest::Client::builder().user_agent(crate::feed::USER_AGENT).build().unwrap();
        let found = super::search(&client, "rust", "fr").await.unwrap();
        assert!(!found.is_empty());
        let top = super::top(&client, "fr").await.unwrap();
        println!("top fr: {} → {:?}", top.len(), top.iter().take(3).map(|r| &r.title).collect::<Vec<_>>());
        let url = found[0].feed_url.clone().unwrap();
        let (p, eps) = crate::feed::fetch(&client, &url).await.unwrap();
        println!("{} — {} épisodes, 1er : {:?} ({:?} s)", p.title, eps.len(), eps[0].title, eps[0].duration);
        assert!(!eps.is_empty());
    }
}
