use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Session status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Ok,
    Exited,
    Crashed,
    Abnormal,
}

/// Session attributes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionAttributes {
    pub release: Option<String>,
    pub environment: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
}

/// A single session update (envelope item type: "session").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionUpdate {
    /// Session ID.
    pub sid: Uuid,
    /// Distinct ID (user identifier).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub did: Option<String>,
    /// Sequence number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequence: Option<u64>,
    /// Timestamp of this update.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    /// When the session started.
    pub started: String,
    /// Whether this is the initial session update.
    #[serde(default)]
    pub init: bool,
    /// Duration in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    /// Session status.
    pub status: SessionStatus,
    /// Number of errors in this session.
    #[serde(default)]
    pub errors: u64,
    /// Session attributes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attrs: Option<SessionAttributes>,
}

/// Aggregated session counts (envelope item type: "sessions").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionAggregates {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attrs: Option<SessionAttributes>,
    pub aggregates: Vec<SessionAggregation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionAggregation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exited: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub errored: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abnormal: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crashed: Option<u64>,
}
