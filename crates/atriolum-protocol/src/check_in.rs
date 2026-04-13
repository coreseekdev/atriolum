use serde::{Deserialize, Serialize};

/// Monitor check-in status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckInStatus {
    Ok,
    Error,
    InProgress,
}

/// A cron monitor check-in (envelope item type: "check_in").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckIn {
    /// Check-in ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_in_id: Option<String>,
    /// Monitor slug / name.
    pub monitor_slug: String,
    /// Status of the check-in.
    pub status: CheckInStatus,
    /// Duration in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    /// Monitor configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monitor_config: Option<MonitorConfig>,
    /// Release version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<String>,
    /// Environment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
}

/// Monitor schedule configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorConfig {
    pub schedule: MonitorSchedule,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkin_margin: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_runtime: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum MonitorSchedule {
    #[serde(rename = "crontab")]
    Crontab(String),
    #[serde(rename = "interval")]
    Interval { value: u64, unit: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_check_in() {
        let json = r#"{
            "check_in_id": "abc123",
            "monitor_slug": "my-cron-job",
            "status": "ok",
            "duration": 5.2,
            "release": "1.0.0"
        }"#;
        let ci: CheckIn = serde_json::from_str(json).unwrap();
        assert_eq!(ci.monitor_slug, "my-cron-job");
        assert_eq!(ci.status, CheckInStatus::Ok);
    }
}
