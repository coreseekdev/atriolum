use atriolum_store::Store;
use futures_util::{SinkExt, StreamExt};
use hyper_util::rt::TokioIo;
use std::sync::Arc;
use tokio_tungstenite::tungstenite;

use crate::AppState;

/// WebSocket target type.
#[derive(Debug, Clone, Copy)]
pub enum WsTarget {
    Cli,
    Term,
}

/// Handle an upgraded WebSocket connection for /ws/cli.
pub async fn handle_cli_ws(
    stream: hyper::upgrade::Upgraded,
    state: Arc<AppState>,
) {
    let io = TokioIo::new(stream);
    let ws_stream = tokio_tungstenite::WebSocketStream::from_raw_socket(
        io,
        tungstenite::protocol::Role::Server,
        None,
    )
    .await;

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Handle incoming messages
    while let Some(msg) = ws_receiver.next().await {
        match msg {
            Ok(tungstenite::Message::Text(text)) => {
                let request: serde_json::Value = match serde_json::from_str(&text) {
                    Ok(v) => v,
                    Err(e) => {
                        let err = serde_json::json!({"type": "error", "message": format!("invalid JSON: {e}")});
                        let _ = ws_sender
                            .send(tungstenite::Message::Text(err.to_string().into()))
                            .await;
                        continue;
                    }
                };

                let msg_type = request["type"].as_str().unwrap_or("");
                let response = match msg_type {
                    "ping" => serde_json::json!({"type": "pong"}),
                    "tail_subscribe" => {
                        // For tail_subscribe, we'll start streaming events
                        // The client enters a receive-only loop
                        serde_json::json!({"type": "tail_subscribed"})
                    }
                    "events_list" => {
                        handle_ws_events_list(&request, &state).await
                    }
                    "events_show" => {
                        handle_ws_events_show(&request, &state).await
                    }
                    "projects_list" => {
                        handle_ws_projects_list(&state).await
                    }
                    "projects_create" => {
                        handle_ws_projects_create(&request, &state).await
                    }
                    "stats" => {
                        handle_ws_stats(&request, &state).await
                    }
                    _ => {
                        serde_json::json!({"type": "error", "message": format!("unknown command: {msg_type}")})
                    }
                };

                let resp_text = serde_json::to_string(&response).unwrap_or_default();
                if ws_sender
                    .send(tungstenite::Message::Text(resp_text.into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            Ok(tungstenite::Message::Close(_)) => break,
            Err(e) => {
                tracing::debug!("WebSocket error: {e}");
                break;
            }
            _ => {}
        }
    }
}

async fn handle_ws_events_list(request: &serde_json::Value, state: &Arc<AppState>) -> serde_json::Value {
    let project = request["project"].as_str().unwrap_or("1");
    let limit = request["limit"].as_u64().unwrap_or(20) as usize;

    let mut filter = atriolum_store::EventFilter {
        limit: Some(limit),
        ..Default::default()
    };
    if let Some(level) = request["level"].as_str() {
        filter.level = Some(level.to_string());
    }

    match state.store.list_events(project, filter).await {
        Ok(events) => {
            let summaries: Vec<serde_json::Value> = events
                .iter()
                .map(|e| serde_json::to_value(e).unwrap_or_default())
                .collect();
            serde_json::json!({"type": "events", "data": summaries})
        }
        Err(e) => serde_json::json!({"type": "error", "message": e.to_string()}),
    }
}

async fn handle_ws_events_show(request: &serde_json::Value, state: &Arc<AppState>) -> serde_json::Value {
    let event_id = match request["event_id"].as_str() {
        Some(id) => id,
        None => return serde_json::json!({"type": "error", "message": "missing event_id"}),
    };
    let project = request["project"].as_str().unwrap_or("1");

    match state.store.get_event(project, event_id).await {
        Ok(Some(event)) => {
            serde_json::json!({"type": "event_detail", "data": event})
        }
        Ok(None) => serde_json::json!({"type": "error", "message": "event not found"}),
        Err(e) => serde_json::json!({"type": "error", "message": e.to_string()}),
    }
}

async fn handle_ws_projects_list(state: &Arc<AppState>) -> serde_json::Value {
    match state.store.list_projects().await {
        Ok(projects) => {
            let vals: Vec<serde_json::Value> = projects
                .iter()
                .map(|p| serde_json::to_value(p).unwrap_or_default())
                .collect();
            serde_json::json!({"type": "projects", "data": vals})
        }
        Err(e) => serde_json::json!({"type": "error", "message": e.to_string()}),
    }
}

async fn handle_ws_projects_create(request: &serde_json::Value, state: &Arc<AppState>) -> serde_json::Value {
    let name = match request["name"].as_str() {
        Some(n) => n,
        None => return serde_json::json!({"type": "error", "message": "missing name"}),
    };
    let public_key = request["public_key"].as_str().unwrap_or("auto-generated-key");
    let project_id = request["project_id"].as_str().unwrap_or("auto");

    let pid = if project_id == "auto" {
        uuid::Uuid::new_v4().to_string()
    } else {
        project_id.to_string()
    };

    match state.store.ensure_project(&pid, name, public_key).await {
        Ok(config) => serde_json::json!({"type": "project", "data": config}),
        Err(e) => serde_json::json!({"type": "error", "message": e.to_string()}),
    }
}

async fn handle_ws_stats(request: &serde_json::Value, state: &Arc<AppState>) -> serde_json::Value {
    let project = request["project"].as_str().unwrap_or("1");

    match state.store.get_project_stats(project).await {
        Ok(stats) => serde_json::json!({"type": "stats", "data": stats}),
        Err(e) => serde_json::json!({"type": "error", "message": e.to_string()}),
    }
}
