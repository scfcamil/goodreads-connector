use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct SearchResp {
    docs: Vec<SearchDoc>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SearchDoc {
    pub title: Option<String>,
    pub author_name: Option<Vec<String>>,
    pub isbn: Option<Vec<String>>,
}

pub async fn search_openlibrary(query: &str, limit: usize) -> Result<Vec<SearchDoc>> {
    let client = Client::new();
    let url = format!("https://openlibrary.org/search.json?q={}&limit={}", urlencoding::encode(query), limit);
    let resp = client.get(&url).send().await?;
    let parsed: SearchResp = resp.json().await?;
    Ok(parsed.docs)
}
