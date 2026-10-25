use crate::output::Activity;
use regex::Regex;
use reqwest::Client;
use rss::Channel;

const MAX_PAGES: u32 = 1000;

pub async fn fetch_and_parse(url: &str) -> anyhow::Result<Vec<Activity>> {
    if url.starts_with("http://") || url.starts_with("https://") {
        fetch_all_pages(url).await
    } else {
        let body = tokio::fs::read_to_string(url).await?;
        parse_feed(&body)
    }
}

async fn fetch_all_pages(base_url: &str) -> anyhow::Result<Vec<Activity>> {
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (compatible; goodreads-connector/0.1)")
        .build()?;
    let sep = if base_url.contains('?') { '&' } else { '?' };
    let mut all_activities = Vec::new();
    for page in 1..=MAX_PAGES {
        let url = format!("{base_url}{sep}page={page}");
        let resp = client.get(&url).send().await?;
        let body = resp.text().await?;
        let activities = parse_feed(&body)?;
        if activities.is_empty() {
            break;
        }
        all_activities.extend(activities);
    }
    Ok(all_activities)
}

pub fn parse_feed(body: &str) -> anyhow::Result<Vec<Activity>> {
    let channel = Channel::read_from(body.as_bytes())?;
    let mut activities = Vec::new();
    let re_book = Regex::new(r"/book/show/(\d+)")?;
    let re_img = Regex::new(r#"<img[^>]+src=\"([^\"]+)\""#)?;
    for item in channel.items() {
        let title = item.title().unwrap_or_default().to_string();
        let link = item.link().unwrap_or_default().to_string();
        let pub_date = item.pub_date().map(|s| s.to_string());
        let desc = item.content().or(item.description()).unwrap_or_default().to_string();
        let book_id = re_book
            .captures(&desc)
            .and_then(|c| c.get(1).map(|m| m.as_str().parse::<u64>().ok()))
            .flatten();
        let cover = re_img
            .captures(&desc)
            .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));
        let activity = Activity {
            title,
            link,
            pub_date,
            description: desc,
            book_id,
            cover,
        };
        activities.push(activity);
    }
    Ok(activities)
}
