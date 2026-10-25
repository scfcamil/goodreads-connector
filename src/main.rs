use clap::{Parser, Subcommand};
use goodreads_connector::{books, enrich, output, parsers, server};
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about = "Goodreads connector: read RSS activity, export JSON, serve MCP endpoints")]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand)]
enum Commands {
    FetchRss { url: String, out: Option<PathBuf> },
    Serve {
        host: Option<String>,
        port: Option<u16>,
        #[arg(long)]
        rss_url: Option<String>,
    },
    FetchBook { book_id: u64, out: Option<PathBuf> },
    Search { query: String, limit: Option<usize> },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();
    match cli.cmd {
        Commands::FetchRss { url, out } => {
            let activities = parsers::rss::fetch_and_parse(&url).await?;
            output::write_json_export(&activities, out.as_ref())?;
        }
        Commands::FetchBook { book_id, out } => {
            let book = books::fetch_and_enrich(book_id).await?;
            output::write_enriched_book(&book, out.as_ref())?;
        }
        Commands::Serve { host, port, rss_url } => {
            let host = host.unwrap_or_else(|| "127.0.0.1".to_string());
            let port = port.unwrap_or(8080);
            let config = server::ServeConfig {
                rss_url,
                bearer_token: std::env::var("MCP_BEARER_TOKEN").ok(),
            };
            server::run_server((host, port), config).await?;
        }
        Commands::Search { query, limit } => {
            let results = enrich::search_openlibrary(&query, limit.unwrap_or(10)).await?;
            println!("{}", serde_json::to_string_pretty(&results)?);
        }
    }
    Ok(())
}
