use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Severity level for structured logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

/// A single structured log entry (from the "log" envelope item type).
///
/// SDKs may send individual logs or batches. The envelope item content type is
/// `application/vnd.sentry.items.log+json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Timestamp (Unix epoch float or RFC 3339 string). May be absent in batch items.
    #[serde(default, deserialize_with = "deserialize_log_timestamp", skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<f64>,

    /// Log severity level.
    pub level: LogLevel,

    /// The log message body.
    pub body: String,

    /// Optional trace ID for correlation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,

    /// Optional span ID for correlation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span_id: Option<String>,

    /// Structured attributes / metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attributes: Option<HashMap<String, serde_json::Value>>,

    /// Severity number (OTel-compatible).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity_number: Option<u32>,
}

/// A batch of log entries (envelope item type: "log").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogBatch {
    pub items: Vec<LogEntry>,
}

fn deserialize_log_timestamp<'de, D>(de: D) -> Result<Option<f64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // Handle both missing field and present value
    let val = Option::<serde_json::Value>::deserialize(de)?;
    match val {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(n)) => Ok(n.as_f64()),
        Some(serde_json::Value::String(s)) => s.parse().ok().map(Some).ok_or_else(|| {
            serde::de::Error::custom("invalid log timestamp string")
        }),
        _ => Err(serde::de::Error::custom("timestamp must be number or string")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_log_entry() {
        let json = r#"{
            "timestamp": 1704067200.0,
            "level": "info",
            "body": "user logged in",
            "trace_id": "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4",
            "attributes": {"user.id": "123"}
        }"#;
        let entry: LogEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.body, "user logged in");
        assert_eq!(entry.level, LogLevel::Info);
        assert!(entry.timestamp.is_some());
    }

    #[test]
    fn test_parse_log_batch() {
        let json = r#"{
            "items": [
                {"level": "error", "body": "connection failed"},
                {"level": "info", "body": "retrying"}
            ]
        }"#;
        let batch: LogBatch = serde_json::from_str(json).unwrap();
        assert_eq!(batch.items.len(), 2);
    }
}
