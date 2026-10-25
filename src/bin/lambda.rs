// Entry point for running the MCP server on AWS Lambda instead of a
// local TCP port. It reuses the exact same axum Router that `serve`
// builds in src/server.rs — only how the router is *run* differs:
// lambda_http::run wires it to Lambda's event source instead of
// axum::serve + a TcpListener.
use goodreads_connector::server::{build_router, ServeConfig};

#[tokio::main]
async fn main() -> Result<(), lambda_http::Error> {
    tracing_subscriber::fmt().without_time().init();

    let config = ServeConfig {
        rss_url: std::env::var("RSS_URL").ok(),
        bearer_token: std::env::var("MCP_BEARER_TOKEN").ok(),
    };
    let app = build_router(config);
    lambda_http::run(app).await
}
