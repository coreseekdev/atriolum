use async_trait::async_trait;
use atriolum_protocol::{Event, EventSummary, ProjectConfig};
use crate::error::StoreError;
use crate::query::EventFilter;

#[async_trait]
pub trait Store: Send + Sync + 'static {
    async fn store_event(
        &self,
        project_id: &str,
        event: &Event,
        raw_json: &[u8],
    ) -> Result<(), StoreError>;

    async fn store_transaction(
        &self,
        project_id: &str,
        event: &Event,
        raw_json: &[u8],
    ) -> Result<(), StoreError>;

    async fn store_session(
        &self,
        project_id: &str,
        session_json: &[u8],
    ) -> Result<(), StoreError>;

    async fn store_attachment(
        &self,
        project_id: &str,
        event_id: &str,
        filename: &str,
        data: &[u8],
    ) -> Result<(), StoreError>;

    async fn store_client_report(
        &self,
        project_id: &str,
        report_json: &[u8],
    ) -> Result<(), StoreError>;

    /// Store structured log entries (appended as JSONL).
    async fn store_logs(
        &self,
        project_id: &str,
        log_json: &[u8],
    ) -> Result<(), StoreError>;

    /// Store a span (appended as JSONL).
    async fn store_span(
        &self,
        project_id: &str,
        span_json: &[u8],
    ) -> Result<(), StoreError>;

    /// Store a check-in (appended as JSONL).
    async fn store_check_in(
        &self,
        project_id: &str,
        check_in_json: &[u8],
    ) -> Result<(), StoreError>;

    /// Store a profile (raw JSON).
    async fn store_profile(
        &self,
        project_id: &str,
        profile_json: &[u8],
    ) -> Result<(), StoreError>;

    /// Store replay event data.
    async fn store_replay(
        &self,
        project_id: &str,
        replay_id: &str,
        data: &[u8],
    ) -> Result<(), StoreError>;

    /// Store raw data for any unrecognized item type.
    async fn store_raw(
        &self,
        project_id: &str,
        item_type: &str,
        data: &[u8],
    ) -> Result<(), StoreError>;

    async fn get_project_config(
        &self,
        project_id: &str,
    ) -> Result<Option<ProjectConfig>, StoreError>;

    async fn list_projects(&self) -> Result<Vec<ProjectConfig>, StoreError>;

    async fn list_events(
        &self,
        project_id: &str,
        filter: EventFilter,
    ) -> Result<Vec<EventSummary>, StoreError>;

    async fn get_event(
        &self,
        project_id: &str,
        event_id: &str,
    ) -> Result<Option<Event>, StoreError>;

    async fn ensure_project(
        &self,
        project_id: &str,
        name: &str,
        public_key: &str,
    ) -> Result<ProjectConfig, StoreError>;
}
