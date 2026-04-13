/// WebSocket target type.
#[derive(Debug, Clone, Copy)]
pub enum WsTarget {
    /// JSON-over-WebSocket protocol for native CLI client
    Cli,
    /// Interactive terminal session for xterm.js
    Term,
}

/// WebSocket protocol messages for the CLI client.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum CliRequest {
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "events_list")]
    EventsList {
        project: Option<String>,
        level: Option<String>,
        limit: Option<usize>,
    },
    #[serde(rename = "events_show")]
    EventsShow {
        event_id: String,
        project: Option<String>,
    },
    #[serde(rename = "projects_list")]
    ProjectsList,
    #[serde(rename = "projects_create")]
    ProjectsCreate {
        name: String,
        public_key: String,
    },
    #[serde(rename = "stats")]
    Stats { project: Option<String> },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum CliResponse {
    #[serde(rename = "pong")]
    Pong,
    #[serde(rename = "ok")]
    Ok { message: String },
    #[serde(rename = "events")]
    Events { data: Vec<atriolum_protocol::EventSummary> },
    #[serde(rename = "event_detail")]
    EventDetail { data: serde_json::Value },
    #[serde(rename = "projects")]
    Projects { data: Vec<atriolum_protocol::ProjectConfig> },
    #[serde(rename = "project")]
    Project { data: atriolum_protocol::ProjectConfig },
    #[serde(rename = "error")]
    Error { message: String },
}
