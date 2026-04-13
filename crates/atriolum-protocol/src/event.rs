use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::types::SdkInfo;

/// Event severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Fatal,
    Error,
    Warning,
    Info,
    Debug,
}

impl Default for Level {
    fn default() -> Self {
        Level::Error
    }
}

/// Custom deserializer for timestamps that can be either RFC 3339 string or numeric Unix epoch.
fn deserialize_timestamp<'de, D>(deserializer: D) -> Result<Option<chrono::DateTime<chrono::Utc>>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    let opt = Option::<serde_json::Value>::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(serde_json::Value::String(s)) => {
            chrono::DateTime::parse_from_rfc3339(&s)
                .map(|dt| Some(dt.with_timezone(&chrono::Utc)))
                .or_else(|_| {
                    // Try as a numeric string
                    let n: f64 = s.parse().map_err(Error::custom)?;
                    chrono::DateTime::from_timestamp(n as i64, 0)
                        .map(Some)
                        .ok_or_else(|| Error::custom("invalid timestamp"))
                })
        }
        Some(serde_json::Value::Number(n)) => {
            let ts = if let Some(f) = n.as_f64() {
                let secs = f as i64;
                let nsecs = ((f - secs as f64) * 1_000_000_000.0) as u32;
                chrono::DateTime::from_timestamp(secs, nsecs)
            } else {
                n.as_i64()
                    .and_then(|i| chrono::DateTime::from_timestamp(i, 0))
            };
            Ok(ts)
        }
        Some(_) => Err(Error::custom("timestamp must be string or number")),
    }
}

/// A Sentry event payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub event_id: Option<Uuid>,

    #[serde(
        default,
        deserialize_with = "deserialize_timestamp",
        skip_serializing_if = "Option::is_none"
    )]
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<Level>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logger: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transaction: Option<String>,

    #[serde(default, rename = "server_name", skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dist: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,

    #[serde(default)]
    pub fingerprint: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<HashMap<String, serde_json::Value>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<HashMap<String, serde_json::Value>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modules: Option<HashMap<String, String>>,

    /// Structured log message (logentry format: {message, params}).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logentry: Option<LogEntry>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exception: Option<ExceptionValues>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub breadcrumbs: Option<BreadcrumbValues>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contexts: Option<HashMap<String, serde_json::Value>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<Request>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threads: Option<ThreadValues>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sdk: Option<SdkInfo>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub culprit: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_timestamp: Option<serde_json::Value>,

    /// Transaction spans (for performance transactions).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spans: Option<Vec<serde_json::Value>>,

    /// Measurements (for performance transactions).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measurements: Option<serde_json::Value>,

    /// Transaction info.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transaction_info: Option<serde_json::Value>,

    /// Debug meta (proguard mappings, etc).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug_meta: Option<serde_json::Value>,

    #[serde(default)]
    pub errors: Vec<serde_json::Value>,

    /// Catch-all for fields we don't explicitly handle.
    #[serde(flatten)]
    pub other: HashMap<String, serde_json::Value>,
}

// --- Typed sub-structures for Event fields ---

/// Structured log entry (Sentry `logentry` field).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub message: String,
    #[serde(default)]
    pub params: Vec<serde_json::Value>,
}

/// Wrapper for exception values (Sentry sends `{ "values": [...] }`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExceptionValues {
    pub values: Vec<Exception>,
}

/// A single exception.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Exception {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exc_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stacktrace: Option<Stacktrace>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_stacktrace: Option<Stacktrace>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<serde_json::Value>,
    #[serde(flatten)]
    pub other: HashMap<String, serde_json::Value>,
}

/// Stack trace with frames.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stacktrace {
    pub frames: Vec<Frame>,
}

/// A single stack frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abs_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lineno: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colno: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_line: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_context: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_context: Option<Vec<String>>,
    #[serde(default)]
    pub in_app: Option<bool>,
    #[serde(flatten)]
    pub other: HashMap<String, serde_json::Value>,
}

/// Wrapper for breadcrumb values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreadcrumbValues {
    pub values: Vec<Breadcrumb>,
}

/// A single breadcrumb.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Breadcrumb {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<serde_json::Value>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub breadcrumb_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<Level>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<HashMap<String, serde_json::Value>>,
    #[serde(flatten)]
    pub other: HashMap<String, serde_json::Value>,
}

/// Wrapper for thread values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadValues {
    pub values: Vec<Thread>,
}

/// A single thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stacktrace: Option<Stacktrace>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_stacktrace: Option<Stacktrace>,
    #[serde(default)]
    pub crashed: Option<bool>,
    #[serde(default)]
    pub current: Option<bool>,
    #[serde(flatten)]
    pub other: HashMap<String, serde_json::Value>,
}

/// User information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(flatten)]
    pub other: HashMap<String, serde_json::Value>,
}

/// HTTP request data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_string: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cookies: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    #[serde(flatten)]
    pub other: HashMap<String, serde_json::Value>,
}

impl Event {
    /// Get the event_id, generating one if missing.
    pub fn event_id_or_new(&mut self) -> Uuid {
        if self.event_id.is_none() {
            self.event_id = Some(Uuid::new_v4());
        }
        self.event_id.unwrap()
    }

    /// Get the event timestamp or now.
    pub fn timestamp_or_now(&self) -> chrono::DateTime<chrono::Utc> {
        self.timestamp.unwrap_or_else(chrono::Utc::now)
    }
}

/// Summary for listing events (lighter than full Event).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSummary {
    pub event_id: String,
    pub level: Option<Level>,
    pub platform: Option<String>,
    pub timestamp: Option<String>,
    pub message: Option<String>,
    pub project_id: String,
    pub environment: Option<String>,
    pub release: Option<String>,
    pub exception_type: Option<String>,
    pub culprit: Option<String>,
}

impl EventSummary {
    pub fn from_event(event: &Event, project_id: &str) -> Self {
        let exception_type = event
            .exception
            .as_ref()
            .and_then(|ev| ev.values.first())
            .and_then(|ex| {
                ex.exc_type
                    .clone()
                    .or_else(|| ex.value.clone())
            });

        Self {
            event_id: event
                .event_id
                .map(|id| id.to_string())
                .unwrap_or_default(),
            level: event.level,
            platform: event.platform.clone(),
            timestamp: event.timestamp.map(|t| t.to_rfc3339()),
            message: event.message.clone().or_else(|| {
                event
                    .exception
                    .as_ref()
                    .and_then(|ev| ev.values.first())
                    .and_then(|ex| {
                        match (&ex.exc_type, &ex.value) {
                            (Some(t), Some(v)) => Some(format!("{t}: {v}")),
                            (Some(t), None) => Some(t.clone()),
                            (None, Some(v)) => Some(v.clone()),
                            _ => None,
                        }
                    })
            }),
            project_id: project_id.to_string(),
            environment: event.environment.clone(),
            release: event.release.clone(),
            exception_type,
            culprit: event.culprit.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_deserialize_basic() {
        let json = r#"{
            "event_id": "fc6d8c0c43fc4630ad850ee518f1b9d0",
            "message": "hello world",
            "level": "error",
            "platform": "python",
            "timestamp": "2026-04-13T10:30:00Z"
        }"#;
        let event: Event = serde_json::from_str(json).unwrap();
        assert_eq!(event.message.as_deref(), Some("hello world"));
        assert_eq!(event.level, Some(Level::Error));
        assert_eq!(event.platform.as_deref(), Some("python"));
        assert!(event.timestamp.is_some());
    }

    #[test]
    fn test_event_deserialize_timestamp_numeric() {
        let json = r#"{"event_id": "fc6d8c0c43fc4630ad850ee518f1b9d0", "timestamp": 1713000000.0}"#;
        let event: Event = serde_json::from_str(json).unwrap();
        assert!(event.timestamp.is_some());
    }

    #[test]
    fn test_event_deserialize_timestamp_integer() {
        let json = r#"{"event_id": "fc6d8c0c43fc4630ad850ee518f1b9d0", "timestamp": 1713000000}"#;
        let event: Event = serde_json::from_str(json).unwrap();
        assert!(event.timestamp.is_some());
    }

    #[test]
    fn test_event_unknown_fields_preserved() {
        let json = r#"{"event_id": "fc6d8c0c43fc4630ad850ee518f1b9d0", "custom_field": "custom_value"}"#;
        let event: Event = serde_json::from_str(json).unwrap();
        assert_eq!(
            event.other.get("custom_field").unwrap().as_str(),
            Some("custom_value")
        );
    }

    #[test]
    fn test_level_deserialize() {
        assert_eq!(
            serde_json::from_str::<Level>(r#""fatal""#).unwrap(),
            Level::Fatal
        );
        assert_eq!(
            serde_json::from_str::<Level>(r#""error""#).unwrap(),
            Level::Error
        );
        assert_eq!(
            serde_json::from_str::<Level>(r#""warning""#).unwrap(),
            Level::Warning
        );
        assert_eq!(
            serde_json::from_str::<Level>(r#""info""#).unwrap(),
            Level::Info
        );
        assert_eq!(
            serde_json::from_str::<Level>(r#""debug""#).unwrap(),
            Level::Debug
        );
    }
}
