use async_trait::async_trait;
use atriolum_protocol::{Event, EventSummary, ProjectConfig};
use crate::error::StoreError;
use crate::query::{EventFilter, ProjectStats, ReleaseSummary};

#[async_trait]
pub trait Store: Send + Sync + 'static {
    // --- Write operations ---

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

    /// Store a user report / feedback.
    async fn store_user_report(
        &self,
        project_id: &str,
        report_json: &[u8],
    ) -> Result<(), StoreError>;

    /// Store raw data for any unrecognized item type.
    async fn store_raw(
        &self,
        project_id: &str,
        item_type: &str,
        data: &[u8],
    ) -> Result<(), StoreError>;

    // --- Read operations ---

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

    /// Get a transaction by ID.
    async fn get_transaction(
        &self,
        project_id: &str,
        transaction_id: &str,
    ) -> Result<Option<Event>, StoreError>;

    /// List transactions for a project.
    async fn list_transactions(
        &self,
        project_id: &str,
        filter: EventFilter,
    ) -> Result<Vec<EventSummary>, StoreError>;

    /// Get project-level statistics.
    async fn get_project_stats(
        &self,
        project_id: &str,
    ) -> Result<ProjectStats, StoreError>;

    /// List releases for a project.
    async fn list_releases(
        &self,
        project_id: &str,
    ) -> Result<Vec<ReleaseSummary>, StoreError>;

    /// List attachment filenames for an event.
    async fn list_attachments(
        &self,
        project_id: &str,
        event_id: &str,
    ) -> Result<Vec<String>, StoreError>;

    /// Read attachment data.
    async fn get_attachment(
        &self,
        project_id: &str,
        event_id: &str,
        filename: &str,
    ) -> Result<Option<Vec<u8>>, StoreError>;

    /// Delete a project and all its data.
    async fn delete_project(
        &self,
        project_id: &str,
    ) -> Result<(), StoreError>;

    // --- Project management ---

    async fn ensure_project(
        &self,
        project_id: &str,
        name: &str,
        public_key: &str,
    ) -> Result<ProjectConfig, StoreError>;

    /// Subscribe to new events (returns event JSON as it arrives).
    /// Used by the WebSocket tail endpoint.
    fn subscribe_events(&self, project_id: &str) -> tokio::sync::broadcast::Receiver<String>;
}
