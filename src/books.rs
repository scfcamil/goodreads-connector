use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// A complete book record with Goodreads and OpenLibrary data
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EnrichedBook {
    pub book_id: u64,
    pub title: String,
    pub author: Option<String>,
    pub rating: Option<f32>,
    pub num_ratings: Option<u32>,
    pub description: Option<String>,
    pub cover_url: Option<String>,
    pub goodreads_url: String,
    /// Data from OpenLibrary enrichment
    pub isbn: Option<Vec<String>>,
    pub openlibrary_authors: Option<Vec<String>>,
}

/// Fetch a book from Goodreads HTML and enrich it with OpenLibrary data
pub async fn fetch_and_enrich(book_id: u64) -> Result<EnrichedBook> {
    let mut book = fetch_goodreads_book(book_id).await?;

    // Enrich with OpenLibrary if we have a title and author
    if !book.title.is_empty() {
        if let Some(ref author) = book.author {
            let query = format!("{} {}", book.title, author);
            if let Ok(results) = crate::enrich::search_openlibrary(&query, 1).await {
                if let Some(first) = results.first() {
                    book.isbn = first.isbn.clone();
                    book.openlibrary_authors = first.author_name.clone();
                }
            }
        }
    }

    Ok(book)
}

/// Fetch a single Goodreads book by ID from the HTML page
async fn fetch_goodreads_book(book_id: u64) -> Result<EnrichedBook> {
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (compatible; goodreads-connector/0.1)")
        .build()?;

    let url = format!("https://www.goodreads.com/book/show/{}", book_id);
    let resp = client.get(&url).send().await?;
    
    if !resp.status().is_success() {
        return Err(anyhow!("Failed to fetch book {}: {}", book_id, resp.status()));
    }

    let html = resp.text().await?;
    parse_goodreads_html(&html, book_id, &url)
}

/// Parse Goodreads book page HTML
fn parse_goodreads_html(html: &str, book_id: u64, url: &str) -> Result<EnrichedBook> {
    use scraper::Html;

    let document = Html::parse_document(html);

    // Title: Look for h1 with class or various title selectors
    let title = get_text_from_selectors(&document, &[
        "h1",
        "[data-testid='bookTitle']",
        ".BookTitle__title",
        ".Text.Heading",
    ]);

    // Author: Look for author links
    let author = get_text_from_selectors(&document, &[
        "a[href*='/author/'] span",
        "[data-testid='bookAuthors'] a",
        ".ContributorLink__name",
    ]);

    // Rating: Look for rating elements
    let rating = extract_rating(&document)?;
    let num_ratings = extract_num_ratings(&document)?;

    // Description: Look in various description containers
    let description = get_text_from_selectors(&document, &[
        "[data-testid='description']",
        ".BookDescription",
        ".BookDetails .description",
    ]);

    // Cover URL: Look for book cover images
    let cover_url = get_attr_from_selectors(&document, &[
        "img[alt='book cover']",
        "[data-testid='bookCover'] img",
        ".BookCover img",
    ], "src");

    Ok(EnrichedBook {
        book_id,
        title: title.unwrap_or_else(|| "Unknown".to_string()),
        author,
        rating,
        num_ratings,
        description,
        cover_url,
        goodreads_url: url.to_string(),
        isbn: None,
        openlibrary_authors: None,
    })
}

/// Helper: Try multiple selectors and return first match
fn get_text_from_selectors(document: &scraper::Html, selectors: &[&str]) -> Option<String> {
    use scraper::Selector;
    
    for selector_str in selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            if let Some(el) = document.select(&selector).next() {
                let text = el
                    .text()
                    .collect::<String>()
                    .trim()
                    .to_string();
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }
    }
    None
}

/// Helper: Try multiple selectors and return first attribute match
fn get_attr_from_selectors(
    document: &scraper::Html,
    selectors: &[&str],
    attr: &str,
) -> Option<String> {
    use scraper::Selector;
    
    for selector_str in selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            if let Some(el) = document.select(&selector).next() {
                if let Some(value) = el.value().attr(attr) {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

fn extract_rating(document: &scraper::Html) -> Result<Option<f32>> {
    // Look for rating in various places
    let rating_text = get_text_from_selectors(document, &[
        "[data-testid='ratingValue']",
        ".RatingStatistics__rating",
        "[role='img']",
    ]);

    let rating = rating_text.and_then(|text| {
        text.split_whitespace()
            .next()
            .and_then(|s| s.parse::<f32>().ok())
    });

    Ok(rating)
}

fn extract_num_ratings(document: &scraper::Html) -> Result<Option<u32>> {
    // Look for number of ratings
    let rating_count_text = get_text_from_selectors(document, &[
        "[data-testid='ratingsCount']",
        ".RatingStatistics__count",
    ]);

    let num_ratings = rating_count_text.and_then(|text| {
        text.chars()
            .filter(|c| c.is_numeric())
            .collect::<String>()
            .parse::<u32>()
            .ok()
    });

    Ok(num_ratings)
}

/// Fetch multiple books from Goodreads and enrich them
pub async fn fetch_and_enrich_multiple(book_ids: Vec<u64>) -> Result<Vec<EnrichedBook>> {
    let mut books = Vec::new();
    for book_id in book_ids {
        match fetch_and_enrich(book_id).await {
            Ok(book) => books.push(book),
            Err(e) => {
                eprintln!("Failed to fetch book {}: {}", book_id, e);
            }
        }
    }
    Ok(books)
}
