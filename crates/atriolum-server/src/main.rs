mod cli;
mod minidump;
mod router;
mod ws;

use atriolum_ingest::{decompress_body, validate_auth, IngestProcessor, MAX_COMPRESSED_SIZE};
use atriolum_ingest::processor::wrap_event_as_envelope;
use atriolum_protocol::parse_envelope;
use atriolum_store::{EventFilter, FilesystemStore, Store};

use clap::Parser;
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

type BoxBody = Full<Bytes>;

fn full_body(data: impl Into<Bytes>) -> BoxBody {
    Full::new(data.into())
}

fn json_response(status: StatusCode, body: &str) -> Response<BoxBody> {
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .header("x-sentry-rate-limits", "")
        .body(full_body(body.to_string()))
        .unwrap()
}

fn error_response(status: StatusCode, detail: &str) -> Response<BoxBody> {
    let body = serde_json::json!({"detail": detail}).to_string();
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .header("x-sentry-error", detail)
        .header("x-sentry-rate-limits", "")
        .body(full_body(body))
        .unwrap()
}

fn cors_response() -> Response<BoxBody> {
    Response::builder()
        .status(StatusCode::OK)
        .header("access-control-allow-origin", "*")
        .header("access-control-allow-methods", "POST, OPTIONS")
        .header(
            "access-control-allow-headers",
            "X-Sentry-Auth, Content-Type, Content-Encoding",
        )
        .body(full_body(""))
        .unwrap()
}

fn add_cors_headers(mut resp: Response<BoxBody>) -> Response<BoxBody> {
    let headers = resp.headers_mut();
    headers.insert("access-control-allow-origin", "*".parse().unwrap());
    headers.insert(
        "access-control-allow-methods",
        "POST, OPTIONS".parse().unwrap(),
    );
    headers.insert(
        "access-control-allow-headers",
        "X-Sentry-Auth, Content-Type, Content-Encoding".parse().unwrap(),
    );
    resp
}

/// Shared server state.
struct AppState {
    store: FilesystemStore,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("atriolum=info".parse()?))
        .init();

    let cli = cli::Cli::parse();

    let store = FilesystemStore::new(cli.data_dir.clone());
    store.init().await?;

    let state = Arc::new(AppState { store });

    let listener = TcpListener::bind((cli.host.as_str(), cli.port)).await?;
    tracing::info!("atriolum server listening on {}:{}", cli.host, cli.port);

    loop {
        let (stream, _remote) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let state = state.clone();

        tokio::spawn(async move {
            let service = service_fn(move |req| {
                let state = state.clone();
                async move { handle_request(req, &state).await }
            });

            if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                tracing::error!("connection error: {err}");
            }
        });
    }
}

async fn handle_request(
    req: Request<hyper::body::Incoming>,
    state: &Arc<AppState>,
) -> anyhow::Result<Response<BoxBody>> {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let query = req.uri().query().unwrap_or("").to_string();

    // CORS preflight
    if method == Method::OPTIONS {
        return Ok(cors_response());
    }

    // Parse path segments
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    // Route: GET /api/health
    if method == Method::GET && parts == ["api", "health"] {
        return Ok(json_response(StatusCode::OK, "{\"status\":\"ok\"}"));
    }

    // Route: WebSocket /ws/cli
    if method == Method::GET && parts == ["ws", "cli"] {
        return handle_ws_upgrade(req, state, ws::WsTarget::Cli).await;
    }

    // Route: WebSocket /ws/term
    if method == Method::GET && parts == ["ws", "term"] {
        return handle_ws_upgrade(req, state, ws::WsTarget::Term).await;
    }

    // --- Management API (/api/projects/...) ---
    if parts.len() >= 2 && parts[0] == "api" && parts[1] == "projects" {
        return Ok(handle_management_api(method, parts, req, state, &query).await);
    }

    // --- SDK Ingest endpoints ---
    if method == Method::POST && parts.len() >= 3 && parts[0] == "api" {
        let project_id = parts[1];
        let endpoint = parts[2];

        match endpoint {
            "envelope" => {
                let resp = handle_envelope(req, state, project_id, &query).await;
                return Ok(add_cors_headers(resp));
            }
            "store" => {
                let resp = handle_store(req, state, project_id, &query).await;
                return Ok(add_cors_headers(resp));
            }
            "minidump" => {
                let resp = handle_minidump(req, state, project_id, &query).await;
                return Ok(add_cors_headers(resp));
            }
            "chunk-upload" => {
                let resp = handle_chunk_upload(req, state, project_id, &query).await;
                return Ok(add_cors_headers(resp));
            }
            _ => {}
        }
    }

    Ok(error_response(StatusCode::NOT_FOUND, "not found"))
}

/// Handle management API requests (/api/projects/...).
async fn handle_management_api(
    method: Method,
    parts: Vec<&str>,
    req: Request<hyper::body::Incoming>,
    state: &Arc<AppState>,
    query: &str,
) -> Response<BoxBody> {
    // GET /api/projects/
    if method == Method::GET && parts == ["api", "projects"] {
        return api_list_projects(state).await;
    }

    // POST /api/projects/
    if method == Method::POST && parts == ["api", "projects"] {
        return api_create_project(req, state).await;
    }

    // Routes with project_id: /api/projects/{id}/...
    if parts.len() >= 3 && parts[0] == "api" && parts[1] == "projects" {
        let project_id = parts[2];

        // GET /api/projects/{id}/
        if method == Method::GET && parts.len() == 3 {
            return api_get_project(state, project_id).await;
        }

        // DELETE /api/projects/{id}/
        if method == Method::DELETE && parts.len() == 3 {
            return api_delete_project(state, project_id).await;
        }

        // Sub-resources under project
        if parts.len() >= 4 {
            let resource = parts[3];

            match resource {
                // GET /api/projects/{id}/events/
                "events" if method == Method::GET && parts.len() == 4 => {
                    return api_list_events(state, project_id, query).await;
                }
                // GET /api/projects/{id}/events/{eid}/
                "events" if method == Method::GET && parts.len() == 5 => {
                    return api_get_event(state, project_id, parts[4]).await;
                }
                // GET /api/projects/{id}/transactions/
                "transactions" if method == Method::GET && parts.len() == 4 => {
                    return api_list_transactions(state, project_id, query).await;
                }
                // GET /api/projects/{id}/stats/
                "stats" if method == Method::GET && parts.len() == 4 => {
                    return api_get_stats(state, project_id).await;
                }
                // GET /api/projects/{id}/releases/
                "releases" if method == Method::GET && parts.len() == 4 => {
                    return api_list_releases(state, project_id).await;
                }
                // GET /api/projects/{id}/attachments/{eid}/
                "attachments" if method == Method::GET && parts.len() == 5 => {
                    return api_list_attachments(state, project_id, parts[4]).await;
                }
                _ => {}
            }
        }
    }

    error_response(StatusCode::NOT_FOUND, "not found")
}

/// Handle multipart minidump upload from C++ SDK.
async fn handle_minidump(
    req: Request<hyper::body::Incoming>,
    state: &Arc<AppState>,
    project_id: &str,
    query: &str,
) -> Response<BoxBody> {
    let headers = req.headers().clone();

    let body_bytes = match collect_body(req.into_body(), MAX_COMPRESSED_SIZE).await {
        Ok(bytes) => bytes,
        Err(e) => return error_response(StatusCode::PAYLOAD_TOO_LARGE, &e.to_string()),
    };

    // Auth
    let auth_header = headers
        .get("x-sentry-auth")
        .and_then(|v| v.to_str().ok());

    let sentry_key = extract_sentry_key(auth_header, query, None);
    let project_config = match state
        .store
        .ensure_project(project_id, project_id, sentry_key.unwrap_or_default().as_str())
        .await
    {
        Ok(config) => config,
        Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("failed to init project: {e}")),
    };

    if let Err(e) = validate_auth(auth_header, query, None, &project_config) {
        return error_response(StatusCode::FORBIDDEN, &e.to_string());
    }

    // Parse Content-Type to get boundary
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let boundary = match minidump::extract_boundary(content_type) {
        Some(b) => b,
        None => return error_response(StatusCode::BAD_REQUEST, "missing multipart boundary"),
    };

    // Parse multipart
    let parts = match minidump::parse_multipart(&body_bytes, &boundary) {
        Ok(parts) => parts,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("multipart parse error: {e}")),
    };

    // Extract minidump and event data
    let mut minidump_data: Option<&[u8]> = None;
    let mut event_json: Option<&[u8]> = None;

    for part in &parts {
        match part.name.as_str() {
            "upload_file_minidump" | "upload_file_minidump_handle" => {
                minidump_data = Some(&part.data);
            }
            "sentry" => {
                event_json = Some(&part.data);
            }
            _ => {}
        }
    }

    // Build an envelope from the parts
    let _event_id = event_json
        .and_then(|d| serde_json::from_slice::<serde_json::Value>(d).ok())
        .and_then(|v| v.get("event_id").cloned())
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    let event_id_uuid = event_json
        .and_then(|d| serde_json::from_slice::<atriolum_protocol::Event>(d).ok())
        .and_then(|e| e.event_id);

    // Build synthetic event payload
    let event_payload = event_json
        .map(|d| d.to_vec())
        .unwrap_or_else(|| br#"{"message":"minidump crash report"}"#.to_vec());

    let mut items = Vec::new();

    // Event item
    let event_id_str = event_id_uuid
        .map(|id| id.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    items.push(atriolum_protocol::EnvelopeItem {
        header: atriolum_protocol::ItemHeader {
            item_type: "event".to_string(),
            length: Some(event_payload.len()),
            content_type: Some("application/json".to_string()),
            filename: None,
            attachment_type: None,
        },
        payload: bytes::Bytes::copy_from_slice(&event_payload),
    });

    // Minidump attachment
    if let Some(dump) = minidump_data {
        items.push(atriolum_protocol::EnvelopeItem {
            header: atriolum_protocol::ItemHeader {
                item_type: "attachment".to_string(),
                length: Some(dump.len()),
                content_type: Some("application/octet-stream".to_string()),
                filename: Some("minidump.dmp".to_string()),
                attachment_type: Some("event.minidump".to_string()),
            },
            payload: bytes::Bytes::copy_from_slice(dump),
        });
    }

    // Attach any other parts as attachments
    for part in &parts {
        if part.name != "upload_file_minidump"
            && part.name != "upload_file_minidump_handle"
            && part.name != "sentry"
        {
            items.push(atriolum_protocol::EnvelopeItem {
                header: atriolum_protocol::ItemHeader {
                    item_type: "attachment".to_string(),
                    length: Some(part.data.len()),
                    content_type: part.content_type.clone(),
                    filename: Some(part.name.clone()),
                    attachment_type: Some("event.attachment".to_string()),
                },
                payload: bytes::Bytes::copy_from_slice(&part.data),
            });
        }
    }

    let envelope = atriolum_protocol::Envelope {
        header: atriolum_protocol::EnvelopeHeader {
            event_id: event_id_uuid,
            dsn: None,
            sdk: None,
            sent_at: None,
        },
        items,
    };

    let processor = IngestProcessor::new(state.store.clone());
    match processor.process_envelope(project_id, envelope).await {
        Ok(_) => json_response(StatusCode::OK, &format!("{{\"id\":\"{event_id_str}\"}}")),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// Handle chunk upload endpoint (for session replay chunks).
async fn handle_chunk_upload(
    req: Request<hyper::body::Incoming>,
    state: &Arc<AppState>,
    project_id: &str,
    query: &str,
) -> Response<BoxBody> {
    let headers = req.headers().clone();

    let body_bytes = match collect_body(req.into_body(), MAX_COMPRESSED_SIZE).await {
        Ok(bytes) => bytes,
        Err(e) => return error_response(StatusCode::PAYLOAD_TOO_LARGE, &e.to_string()),
    };

    let auth_header = headers
        .get("x-sentry-auth")
        .and_then(|v| v.to_str().ok());

    let sentry_key = extract_sentry_key(auth_header, query, None);
    let project_config = match state
        .store
        .ensure_project(project_id, project_id, sentry_key.unwrap_or_default().as_str())
        .await
    {
        Ok(config) => config,
        Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("failed to init project: {e}")),
    };

    if let Err(e) = validate_auth(auth_header, query, None, &project_config) {
        return error_response(StatusCode::FORBIDDEN, &e.to_string());
    }

    // Store chunk as raw data for now
    let chunk_id = uuid::Uuid::new_v4().to_string();
    let _ = state
        .store
        .store_raw(project_id, "chunk_upload", &body_bytes)
        .await;

    json_response(StatusCode::OK, &format!("{{\"id\":\"{chunk_id}\"}}"))
}

// ---- Management API handlers ----

async fn api_list_projects(state: &Arc<AppState>) -> Response<BoxBody> {
    match state.store.list_projects().await {
        Ok(projects) => {
            let body = serde_json::to_string(&projects).unwrap_or_default();
            json_response(StatusCode::OK, &body)
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn api_create_project(
    req: Request<hyper::body::Incoming>,
    state: &Arc<AppState>,
) -> Response<BoxBody> {
    let body_bytes = match collect_body(req.into_body(), 1024 * 1024).await {
        Ok(bytes) => bytes,
        Err(e) => return error_response(StatusCode::PAYLOAD_TOO_LARGE, &e.to_string()),
    };

    let input: HashMap<String, String> = match serde_json::from_slice(&body_bytes) {
        Ok(v) => v,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("invalid JSON: {e}")),
    };

    let name = input.get("name").map(|s| s.as_str()).unwrap_or("unnamed");
    let public_key = input
        .get("public_key")
        .cloned()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let project_id = input
        .get("project_id")
        .cloned()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    match state.store.ensure_project(&project_id, name, &public_key).await {
        Ok(config) => {
            let body = serde_json::to_string(&config).unwrap_or_default();
            json_response(StatusCode::CREATED, &body)
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn api_get_project(state: &Arc<AppState>, project_id: &str) -> Response<BoxBody> {
    match state.store.get_project_config(project_id).await {
        Ok(Some(config)) => {
            let body = serde_json::to_string(&config).unwrap_or_default();
            json_response(StatusCode::OK, &body)
        }
        Ok(None) => error_response(StatusCode::NOT_FOUND, "project not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn api_delete_project(state: &Arc<AppState>, project_id: &str) -> Response<BoxBody> {
    match state.store.delete_project(project_id).await {
        Ok(()) => json_response(StatusCode::OK, "{\"detail\":\"deleted\"}"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn api_list_events(state: &Arc<AppState>, project_id: &str, query: &str) -> Response<BoxBody> {
    let filter = parse_event_filter(query);
    match state.store.list_events(project_id, filter).await {
        Ok(events) => {
            let body = serde_json::to_string(&events).unwrap_or_default();
            json_response(StatusCode::OK, &body)
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn api_get_event(state: &Arc<AppState>, project_id: &str, event_id: &str) -> Response<BoxBody> {
    match state.store.get_event(project_id, event_id).await {
        Ok(Some(event)) => {
            let body = serde_json::to_string(&event).unwrap_or_default();
            json_response(StatusCode::OK, &body)
        }
        Ok(None) => error_response(StatusCode::NOT_FOUND, "event not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn api_list_transactions(state: &Arc<AppState>, project_id: &str, query: &str) -> Response<BoxBody> {
    let filter = parse_event_filter(query);
    match state.store.list_transactions(project_id, filter).await {
        Ok(txs) => {
            let body = serde_json::to_string(&txs).unwrap_or_default();
            json_response(StatusCode::OK, &body)
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn api_get_stats(state: &Arc<AppState>, project_id: &str) -> Response<BoxBody> {
    match state.store.get_project_stats(project_id).await {
        Ok(stats) => {
            let body = serde_json::to_string(&stats).unwrap_or_default();
            json_response(StatusCode::OK, &body)
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn api_list_releases(state: &Arc<AppState>, project_id: &str) -> Response<BoxBody> {
    match state.store.list_releases(project_id).await {
        Ok(releases) => {
            let body = serde_json::to_string(&releases).unwrap_or_default();
            json_response(StatusCode::OK, &body)
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn api_list_attachments(state: &Arc<AppState>, project_id: &str, event_id: &str) -> Response<BoxBody> {
    match state.store.list_attachments(project_id, event_id).await {
        Ok(names) => {
            let body = serde_json::to_string(&names).unwrap_or_default();
            json_response(StatusCode::OK, &body)
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// Parse query string into EventFilter.
fn parse_event_filter(query: &str) -> EventFilter {
    let mut filter = EventFilter::default();
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            let v = urldecode(v);
            match k {
                "level" => filter.level = Some(v),
                "limit" => filter.limit = v.parse().ok(),
                "cursor" => filter.cursor = Some(v),
                "platform" => filter.platform = Some(v),
                "query" => filter.query = Some(v),
                "environment" => filter.environment = Some(v),
                "release" => filter.release = Some(v),
                _ => {}
            }
        }
    }
    filter
}

/// Simple URL percent-decoding.
fn urldecode(s: &str) -> String {
    let mut result = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                result.push(byte);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&result).to_string()
}

// ---- SDK Ingest Handlers ----

async fn handle_envelope(
    req: Request<hyper::body::Incoming>,
    state: &Arc<AppState>,
    project_id: &str,
    query: &str,
) -> Response<BoxBody> {
    // Extract headers before moving body
    let headers = req.headers().clone();

    // Read body with size limit
    let body_bytes = match collect_body(req.into_body(), MAX_COMPRESSED_SIZE).await {
        Ok(bytes) => bytes,
        Err(e) => {
            return error_response(StatusCode::PAYLOAD_TOO_LARGE, &e.to_string());
        }
    };

    // Decompress
    let encoding = headers
        .get("content-encoding")
        .and_then(|v| v.to_str().ok());

    let decompressed = match decompress_body(&body_bytes, encoding) {
        Ok(data) => data,
        Err(e) => {
            return error_response(StatusCode::BAD_REQUEST, &e.to_string());
        }
    };

    // Get or create project config
    let auth_header = headers
        .get("x-sentry-auth")
        .and_then(|v| v.to_str().ok());

    // Parse envelope header to extract DSN for auth
    let envelope_dsn = {
        let first_newline = decompressed
            .iter()
            .position(|&b| b == b'\n')
            .unwrap_or(decompressed.len());
        let header_str = std::str::from_utf8(&decompressed[..first_newline]).unwrap_or("");
        serde_json::from_str::<atriolum_protocol::EnvelopeHeader>(header_str)
            .ok()
            .and_then(|h| h.dsn)
    };

    // Auto-create project if needed
    let sentry_key = extract_sentry_key(auth_header, query, envelope_dsn.as_deref());
    let project_config = match state
        .store
        .ensure_project(project_id, project_id, sentry_key.unwrap_or_default().as_str())
        .await
    {
        Ok(config) => config,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("failed to init project: {e}"),
            );
        }
    };

    // Validate auth
    if let Err(e) = validate_auth(auth_header, query, envelope_dsn.as_deref(), &project_config) {
        return error_response(StatusCode::FORBIDDEN, &e.to_string());
    }

    // Parse envelope
    let envelope = match parse_envelope(&decompressed) {
        Ok(env) => env,
        Err(e) => {
            return error_response(StatusCode::BAD_REQUEST, &e.to_string());
        }
    };

    // Process
    let processor = IngestProcessor::new(state.store.clone());
    match processor.process_envelope(project_id, envelope).await {
        Ok(result) => {
            let id = result
                .event_id
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            json_response(StatusCode::OK, &format!("{{\"id\":\"{id}\"}}"))
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn handle_store(
    req: Request<hyper::body::Incoming>,
    state: &Arc<AppState>,
    project_id: &str,
    query: &str,
) -> Response<BoxBody> {
    let headers = req.headers().clone();

    let body_bytes = match collect_body(req.into_body(), MAX_COMPRESSED_SIZE).await {
        Ok(bytes) => bytes,
        Err(e) => {
            return error_response(StatusCode::PAYLOAD_TOO_LARGE, &e.to_string());
        }
    };

    let encoding = headers
        .get("content-encoding")
        .and_then(|v| v.to_str().ok());

    let decompressed = match decompress_body(&body_bytes, encoding) {
        Ok(data) => data,
        Err(e) => {
            return error_response(StatusCode::BAD_REQUEST, &e.to_string());
        }
    };

    let auth_header = headers
        .get("x-sentry-auth")
        .and_then(|v| v.to_str().ok());

    let sentry_key = extract_sentry_key(auth_header, query, None);
    let project_config = match state
        .store
        .ensure_project(project_id, project_id, sentry_key.unwrap_or_default().as_str())
        .await
    {
        Ok(config) => config,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("failed to init project: {e}"),
            );
        }
    };

    if let Err(e) = validate_auth(auth_header, query, None, &project_config) {
        return error_response(StatusCode::FORBIDDEN, &e.to_string());
    }

    let envelope = match wrap_event_as_envelope(&decompressed) {
        Ok(env) => env,
        Err(e) => {
            return error_response(StatusCode::BAD_REQUEST, &e.to_string());
        }
    };

    let processor = IngestProcessor::new(state.store.clone());
    match processor.process_envelope(project_id, envelope).await {
        Ok(result) => {
            let id = result
                .event_id
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            json_response(StatusCode::OK, &format!("{{\"id\":\"{id}\"}}"))
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// Extract sentry_key from various auth sources (for project auto-creation).
fn extract_sentry_key(
    auth_header: Option<&str>,
    query: &str,
    envelope_dsn: Option<&str>,
) -> Option<String> {
    // Try auth header
    if let Some(header) = auth_header {
        if let Ok(auth) = atriolum_protocol::parse_sentry_auth(header) {
            return Some(auth.sentry_key);
        }
    }
    // Try query string
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == "sentry_key" {
                return Some(v.to_string());
            }
        }
    }
    // Try DSN
    if let Some(dsn_str) = envelope_dsn {
        if let Ok(dsn) = atriolum_protocol::parse_dsn(dsn_str) {
            return Some(dsn.public_key);
        }
    }
    None
}

async fn collect_body(
    body: hyper::body::Incoming,
    max_size: usize,
) -> Result<Vec<u8>, String> {
    let body_bytes = body
        .collect()
        .await
        .map_err(|e| format!("failed to read body: {e}"))?
        .to_bytes();

    if body_bytes.len() > max_size {
        return Err(format!(
            "body too large: {} bytes (max {})",
            body_bytes.len(),
            max_size
        ));
    }

    Ok(body_bytes.to_vec())
}

/// Handle WebSocket upgrade for /ws/cli and /ws/term.
async fn handle_ws_upgrade(
    req: Request<hyper::body::Incoming>,
    state: &Arc<AppState>,
    target: ws::WsTarget,
) -> anyhow::Result<Response<BoxBody>> {
    use tokio_tungstenite::tungstenite::handshake::derive_accept_key;

    let ws_key = req
        .headers()
        .get("sec-websocket-key")
        .ok_or_else(|| anyhow::anyhow!("missing sec-websocket-key"))?;

    let derived = derive_accept_key(ws_key.as_bytes());

    // Spawn a task to handle the upgraded connection
    let state_clone = state.clone();
    tokio::spawn(async move {
        // Wait for the HTTP upgrade to complete
        match hyper::upgrade::on(req).await {
            Ok(upgraded) => {
                match target {
                    ws::WsTarget::Cli => {
                        ws::handle_cli_ws(upgraded, state_clone).await;
                    }
                    ws::WsTarget::Term => {
                        tracing::info!("Terminal WebSocket connected (not yet implemented)");
                    }
                }
            }
            Err(e) => {
                tracing::error!("WebSocket upgrade failed: {e}");
            }
        }
    });

    Ok(Response::builder()
        .status(StatusCode::SWITCHING_PROTOCOLS)
        .header("upgrade", "websocket")
        .header("connection", "upgrade")
        .header("sec-websocket-accept", derived)
        .body(full_body(""))
        .unwrap())
}
