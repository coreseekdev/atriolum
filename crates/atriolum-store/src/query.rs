use serde::{Deserialize, Serialize};

/// Filter parameters for listing events.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventFilter {
    /// Filter by severity level.
    pub level: Option<String>,
    /// Maximum number of results.
    pub limit: Option<usize>,
    /// Cursor for pagination (event_id of last result).
    pub cursor: Option<String>,
    /// Filter by platform (e.g. "python", "javascript").
    pub platform: Option<String>,
    /// Filter by project ID (used when querying across projects).
    pub project: Option<String>,
    /// Full-text search in message, exception type/value, logger.
    pub query: Option<String>,
    /// Start of time range (RFC 3339 or Unix timestamp).
    pub start: Option<String>,
    /// End of time range (RFC 3339 or Unix timestamp).
    pub end: Option<String>,
    /// Filter by environment.
    pub environment: Option<String>,
    /// Filter by release.
    pub release: Option<String>,
    /// Include transactions in results.
    #[serde(default)]
    pub include_transactions: bool,
}

/// Filter for listing transactions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransactionFilter {
    pub limit: Option<usize>,
    pub cursor: Option<String>,
    pub platform: Option<String>,
    pub query: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub environment: Option<String>,
    pub release: Option<String>,
}

/// Project-level statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectStats {
    pub project_id: String,
    pub total_events: u64,
    pub total_transactions: u64,
    pub total_sessions: u64,
    pub events_by_level: std::collections::HashMap<String, u64>,
    pub recent_errors: u64,
    pub last_event_at: Option<String>,
}

/// Release summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseSummary {
    pub release: String,
    pub environment: Option<String>,
    pub event_count: u64,
    pub first_seen: Option<String>,
    pub last_seen: Option<String>,
}
