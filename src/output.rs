use serde::Serialize;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Serialize, Clone)]
pub struct Activity {
    pub title: String,
    pub link: String,
    pub pub_date: Option<String>,
    pub description: String,
    pub book_id: Option<u64>,
    pub cover: Option<String>,
}

#[derive(Serialize)]
struct Export {
    activities: Vec<Activity>,
}

pub fn write_json_export(activities: &Vec<Activity>, out: Option<&PathBuf>) -> anyhow::Result<()> {
    let export = Export { activities: activities.clone() };
    let s = serde_json::to_string_pretty(&export)?;
    match out {
        Some(p) => {
            let mut f = File::create(p)?;
            f.write_all(s.as_bytes())?;
            println!("Wrote {} entries to {}", export.activities.len(), p.display());
        }
        None => {
            println!("{}", s);
        }
    }
    Ok(())
}

pub fn write_enriched_book(book: &crate::books::EnrichedBook, out: Option<&PathBuf>) -> anyhow::Result<()> {
    let s = serde_json::to_string_pretty(&book)?;
    match out {
        Some(p) => {
            let mut f = File::create(p)?;
            f.write_all(s.as_bytes())?;
            println!("Wrote book {} to {}", book.title, p.display());
        }
        None => {
            println!("{}", s);
        }
    }
    Ok(())
}
