use crate::enrich;
use crate::parsers;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Clone)]
pub struct ServeConfig {
    pub rss_url: Option<String>,
    pub bearer_token: Option<String>,
}

#[derive(Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

pub fn build_router(config: ServeConfig) -> Router {
    Router::new()
        .route("/mcp", post(handle_mcp))
        .with_state(Arc::new(config))
}

pub async fn run_server((host, port): (String, u16), config: ServeConfig) -> anyhow::Result<()> {
    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
    let app = build_router(config);
    println!("MCP server listening on http://{}/mcp", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn handle_mcp(
    State(config): State<Arc<ServeConfig>>,
    headers: HeaderMap,
    Json(req): Json<JsonRpcRequest>,
) -> (StatusCode, Json<Value>) {
    let id = req.id.clone().unwrap_or(Value::Null);

    if let Some(expected) = &config.bearer_token {
        let provided = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "));
        if provided != Some(expected.as_str()) {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {"code": -32000, "message": "unauthorized"}
                })),
            );
        }
    }

    if req.jsonrpc != "2.0" {
        return err_response(id, -32600, "invalid jsonrpc version");
    }

    // Notifications carry no id and expect no response body content beyond 202.
    let is_notification = req.id.is_none();

    let result = match req.method.as_str() {
        "initialize" => Ok(json!({
            "protocolVersion": "2025-06-18",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "goodreads-connector", "version": env!("CARGO_PKG_VERSION") }
        })),
        "notifications/initialized" | "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tool_defs() })),
        "tools/call" => handle_tool_call(&config, req.params).await,
        _ => Err((-32601, format!("method not found: {}", req.method))),
    };

    if is_notification {
        return (StatusCode::ACCEPTED, Json(json!({})));
    }

    match result {
        Ok(value) => (
            StatusCode::OK,
            Json(json!({ "jsonrpc": "2.0", "id": id, "result": value })),
        ),
        Err((code, message)) => err_response(id, code, &message),
    }
}

fn err_response(id: Value, code: i64, message: &str) -> (StatusCode, Json<Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": code, "message": message }
        })),
    )
}

fn tool_defs() -> Vec<Value> {
    vec![
        json!({
            "name": "list_activities",
            "description": "List recent activity from the configured Goodreads RSS feed.",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
        }),
        json!({
            "name": "search_books",
            "description": "Search OpenLibrary for books matching a free-text query.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "q": { "type": "string", "description": "search query" },
                    "limit": { "type": "integer", "description": "max results", "default": 10 }
                },
                "required": ["q"],
                "additionalProperties": false
            }
        }),
    ]
}

async fn handle_tool_call(config: &ServeConfig, params: Value) -> Result<Value, (i64, String)> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or((-32602, "missing tool name".to_string()))?;
    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    let payload = match name {
        "list_activities" => {
            let url = config
                .rss_url
                .as_ref()
                .ok_or((-32000, "no rss_url configured for this server".to_string()))?;
            let activities = parsers::rss::fetch_and_parse(url)
                .await
                .map_err(|e| (-32000, format!("failed to fetch rss: {e}")))?;
            json!({ "activities": activities })
        }
        "search_books" => {
            let q = args.get("q").and_then(|v| v.as_str()).unwrap_or_default();
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
            let docs = enrich::search_openlibrary(q, limit)
                .await
                .map_err(|e| (-32000, format!("search failed: {e}")))?;
            json!({ "results": docs })
        }
        other => return Err((-32602, format!("unknown tool: {other}"))),
    };

    Ok(json!({
        "content": [{ "type": "text", "text": serde_json::to_string_pretty(&payload).unwrap() }],
        "isError": false
    }))
}
