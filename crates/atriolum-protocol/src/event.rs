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

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exception: Option<serde_json::Value>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub breadcrumbs: Option<serde_json::Value>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contexts: Option<serde_json::Value>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<serde_json::Value>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<serde_json::Value>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threads: Option<serde_json::Value>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sdk: Option<SdkInfo>,

    #[serde(default)]
    pub errors: Vec<serde_json::Value>,

    /// Catch-all for fields we don't explicitly handle
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
}

impl EventSummary {
    pub fn from_event(event: &Event, project_id: &str) -> Self {
        Self {
            event_id: event
                .event_id
                .map(|id| id.to_string())
                .unwrap_or_default(),
            level: event.level,
            platform: event.platform.clone(),
            timestamp: event.timestamp.map(|t| t.to_rfc3339()),
            message: event.message.clone(),
            project_id: project_id.to_string(),
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
