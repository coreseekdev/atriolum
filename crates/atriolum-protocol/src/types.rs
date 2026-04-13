use serde::{Deserialize, Serialize};

/// SDK info reported in envelope headers and events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkInfo {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub packages: Vec<serde_json::Value>,
}

/// Known envelope item types. Unknown types are handled as raw strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KnownItemType {
    Event,
    Transaction,
    Attachment,
    Session,
    Sessions,
    ClientReport,
    UserReport,
    Feedback,
    Log,
    Span,
    CheckIn,
    Profile,
    ProfileChunk,
    ReplayEvent,
    ReplayRecording,
    Statsd,
    MetricMeta,
    Metric,
    TraceMetric,
    RawSecurity,
    UserFeedback,
}

impl KnownItemType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "event" => Some(Self::Event),
            "transaction" => Some(Self::Transaction),
            "attachment" => Some(Self::Attachment),
            "session" => Some(Self::Session),
            "sessions" => Some(Self::Sessions),
            "client_report" => Some(Self::ClientReport),
            "user_report" => Some(Self::UserReport),
            "feedback" => Some(Self::Feedback),
            "log" => Some(Self::Log),
            "span" => Some(Self::Span),
            "check_in" => Some(Self::CheckIn),
            "profile" => Some(Self::Profile),
            "profile_chunk" => Some(Self::ProfileChunk),
            "replay_event" => Some(Self::ReplayEvent),
            "replay_recording" => Some(Self::ReplayRecording),
            "statsd" => Some(Self::Statsd),
            "metric_meta" => Some(Self::MetricMeta),
            "metric" => Some(Self::Metric),
            "trace_metric" => Some(Self::TraceMetric),
            "raw_security" => Some(Self::RawSecurity),
            "user_feedback" => Some(Self::UserFeedback),
            _ => None,
        }
    }
}

/// Project configuration stored on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project_id: String,
    pub project_name: String,
    pub keys: Vec<ProjectKey>,
}

/// An authentication key for a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectKey {
    pub public_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_key: Option<String>,
}
