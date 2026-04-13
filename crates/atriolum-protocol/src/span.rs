use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single span (envelope item type: "span").
///
/// Spans can be sent individually (from JS/Rust SDKs) or as part of a transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    /// Span ID (16 hex chars).
    pub span_id: String,
    /// Trace ID (32 hex chars).
    pub trace_id: String,
    /// Parent span ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<String>,
    /// Operation name (e.g., "http.server").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op: Option<String>,
    /// Description of the span.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Start timestamp.
    #[serde(default)]
    pub start_timestamp: Option<serde_json::Value>,
    /// End timestamp.
    #[serde(default)]
    pub timestamp: Option<serde_json::Value>,
    /// Span status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<HashMap<String, String>>,
    /// Extra data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<HashMap<String, serde_json::Value>>,
    /// Whether this is the same process as the parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub same_process_as_parent: Option<bool>,
    /// Is this an exclusive time span?
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_segment: Option<bool>,
    /// Origin of the span.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_span() {
        let json = r#"{
            "span_id": "a1b2c3d4e5f67890",
            "trace_id": "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4",
            "parent_span_id": "1234567890abcdef",
            "op": "http.server",
            "description": "GET /api/users",
            "status": "ok",
            "tags": {"http.method": "GET"},
            "start_timestamp": 1704067200.0,
            "timestamp": 1704067201.5
        }"#;
        let span: Span = serde_json::from_str(json).unwrap();
        assert_eq!(span.span_id, "a1b2c3d4e5f67890");
        assert_eq!(span.op.as_deref(), Some("http.server"));
    }
}
