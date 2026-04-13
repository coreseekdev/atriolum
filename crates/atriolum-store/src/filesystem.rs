use async_trait::async_trait;
use atriolum_protocol::{Event, EventSummary, ProjectConfig, ProjectKey};
use chrono::Utc;
use dashmap::DashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tracing;

use crate::error::StoreError;
use crate::query::{EventFilter, ProjectStats, ReleaseSummary};
use crate::store::Store;

/// Default broadcast channel capacity for live event streaming.
const BROADCAST_CAPACITY: usize = 256;

pub struct FilesystemStore {
    base_dir: PathBuf,
    /// Per-project broadcast channels for live event streaming.
    channels: Arc<DashMap<String, tokio::sync::broadcast::Sender<String>>>,
    /// Global broadcast channel for all events.
    global_channel: tokio::sync::broadcast::Sender<String>,
}

impl FilesystemStore {
    pub fn new(base_dir: PathBuf) -> Self {
        let (global_tx, _) = tokio::sync::broadcast::channel(BROADCAST_CAPACITY);
        Self {
            base_dir,
            channels: Arc::new(DashMap::new()),
            global_channel: global_tx,
        }
    }

    /// Initialize the storage directory. Call once at startup.
    pub async fn init(&self) -> Result<(), StoreError> {
        fs::create_dir_all(self.base_dir.join("projects")).await?;
        Ok(())
    }

    fn project_dir(&self, project_id: &str) -> PathBuf {
        self.base_dir.join("projects").join(project_id)
    }

    /// Get the year-month directory for event storage.
    fn month_dir(&self, project_id: &str, dt: &chrono::DateTime<Utc>, kind: &str) -> PathBuf {
        let month = dt.format("%Y-%m").to_string();
        self.project_dir(project_id).join(kind).join(month)
    }

    /// Atomic write: write to temp file, then rename.
    async fn atomic_write(&self, path: &Path, data: &[u8]) -> Result<(), StoreError> {
        let tmp_path = path.with_extension("tmp");
        fs::write(&tmp_path, data).await?;
        fs::rename(&tmp_path, path).await?;
        Ok(())
    }

    /// Append a line to a JSONL file.
    async fn append_jsonl(&self, path: &Path, data: &[u8]) -> Result<(), StoreError> {
        use tokio::io::AsyncWriteExt;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?;
        file.write_all(data).await?;
        file.write_all(b"\n").await?;
        Ok(())
    }

    /// Broadcast an event to subscribers.
    fn broadcast_event(&self, project_id: &str, event_json: &str) {
        // Project-specific channel
        if let Some(tx) = self.channels.get(project_id) {
            let _ = tx.send(event_json.to_string());
        } else {
            let (tx, _) = tokio::sync::broadcast::channel(BROADCAST_CAPACITY);
            let _ = tx.send(event_json.to_string());
            self.channels.insert(project_id.to_string(), tx);
        }
        // Global channel
        let _ = self.global_channel.send(event_json.to_string());
    }
}

impl Clone for FilesystemStore {
    fn clone(&self) -> Self {
        Self {
            base_dir: self.base_dir.clone(),
            channels: self.channels.clone(),
            global_channel: self.global_channel.clone(),
        }
    }
}

/// Check if an event matches a filter's query (text search).
fn matches_query(event: &Event, query: &str) -> bool {
    let query_lower = query.to_lowercase();
    let search_fields = [
        event.message.as_deref(),
        event.logger.as_deref(),
        event.culprit.as_deref(),
        event.transaction.as_deref(),
        event.exception.as_ref().and_then(|ev| ev.values.first()?.exc_type.as_deref()),
        event.exception.as_ref().and_then(|ev| ev.values.first()?.value.as_deref()),
    ];
    search_fields.iter().any(|field| {
        field
            .map(|f| f.to_lowercase().contains(&query_lower))
            .unwrap_or(false)
    })
}

#[async_trait]
impl Store for FilesystemStore {
    async fn store_event(
        &self,
        project_id: &str,
        event: &Event,
        raw_json: &[u8],
    ) -> Result<(), StoreError> {
        let event_id = event
            .event_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let ts = event.timestamp_or_now();
        let dir = self.month_dir(project_id, &ts, "events");
        fs::create_dir_all(&dir).await?;

        let path = dir.join(format!("{event_id}.json"));
        self.atomic_write(&path, raw_json).await?;

        // Broadcast for live tail
        if let Ok(json_str) = std::str::from_utf8(raw_json) {
            self.broadcast_event(project_id, json_str);
        }

        tracing::debug!(%project_id, %event_id, "stored event");
        Ok(())
    }

    async fn store_transaction(
        &self,
        project_id: &str,
        event: &Event,
        raw_json: &[u8],
    ) -> Result<(), StoreError> {
        let event_id = event
            .event_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let ts = event.timestamp_or_now();
        let dir = self.month_dir(project_id, &ts, "transactions");
        fs::create_dir_all(&dir).await?;

        let path = dir.join(format!("{event_id}.json"));
        self.atomic_write(&path, raw_json).await?;

        tracing::debug!(%project_id, %event_id, "stored transaction");
        Ok(())
    }

    async fn store_session(
        &self,
        project_id: &str,
        session_json: &[u8],
    ) -> Result<(), StoreError> {
        let dir = self.project_dir(project_id).join("sessions");
        fs::create_dir_all(&dir).await?;

        let date = Utc::now().format("%Y-%m-%d").to_string();
        let path = dir.join(format!("{date}.jsonl"));

        let mut line = session_json.to_vec();
        line.push(b'\n');

        use tokio::io::AsyncWriteExt;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;
        file.write_all(&line).await?;

        tracing::debug!(%project_id, "stored session");
        Ok(())
    }

    async fn store_attachment(
        &self,
        project_id: &str,
        event_id: &str,
        filename: &str,
        data: &[u8],
    ) -> Result<(), StoreError> {
        let dir = self
            .project_dir(project_id)
            .join("attachments")
            .join(event_id);
        fs::create_dir_all(&dir).await?;

        let path = dir.join(filename);
        self.atomic_write(&path, data).await?;

        tracing::debug!(%project_id, %event_id, %filename, "stored attachment");
        Ok(())
    }

    async fn store_client_report(
        &self,
        project_id: &str,
        report_json: &[u8],
    ) -> Result<(), StoreError> {
        let dir = self.project_dir(project_id).join("client_reports");
        fs::create_dir_all(&dir).await?;

        let id = uuid::Uuid::new_v4().to_string();
        let ts = Utc::now().format("%Y-%m-%d").to_string();
        let path = dir.join(format!("{ts}-{id}.json"));
        self.atomic_write(&path, report_json).await?;

        tracing::debug!(%project_id, "stored client report");
        Ok(())
    }

    async fn store_user_report(
        &self,
        project_id: &str,
        report_json: &[u8],
    ) -> Result<(), StoreError> {
        let dir = self.project_dir(project_id).join("user_reports");
        fs::create_dir_all(&dir).await?;

        let id = uuid::Uuid::new_v4().to_string();
        let ts = Utc::now().format("%Y-%m-%d").to_string();
        let path = dir.join(format!("{ts}-{id}.json"));
        self.atomic_write(&path, report_json).await?;

        tracing::debug!(%project_id, "stored user report/feedback");
        Ok(())
    }

    async fn store_logs(
        &self,
        project_id: &str,
        log_json: &[u8],
    ) -> Result<(), StoreError> {
        let dir = self.project_dir(project_id).join("logs");
        fs::create_dir_all(&dir).await?;
        let date = Utc::now().format("%Y-%m-%d").to_string();
        let path = dir.join(format!("{date}.jsonl"));
        self.append_jsonl(&path, log_json).await?;
        tracing::debug!(%project_id, "stored log entries");
        Ok(())
    }

    async fn store_span(
        &self,
        project_id: &str,
        span_json: &[u8],
    ) -> Result<(), StoreError> {
        let dir = self.project_dir(project_id).join("spans");
        fs::create_dir_all(&dir).await?;
        let date = Utc::now().format("%Y-%m-%d").to_string();
        let path = dir.join(format!("{date}.jsonl"));
        self.append_jsonl(&path, span_json).await?;
        tracing::debug!(%project_id, "stored span");
        Ok(())
    }

    async fn store_check_in(
        &self,
        project_id: &str,
        check_in_json: &[u8],
    ) -> Result<(), StoreError> {
        let dir = self.project_dir(project_id).join("check_ins");
        fs::create_dir_all(&dir).await?;
        let date = Utc::now().format("%Y-%m-%d").to_string();
        let path = dir.join(format!("{date}.jsonl"));
        self.append_jsonl(&path, check_in_json).await?;
        tracing::debug!(%project_id, "stored check-in");
        Ok(())
    }

    async fn store_profile(
        &self,
        project_id: &str,
        profile_json: &[u8],
    ) -> Result<(), StoreError> {
        let dir = self.project_dir(project_id).join("profiles");
        fs::create_dir_all(&dir).await?;
        let id = uuid::Uuid::new_v4().to_string();
        let ts = Utc::now().format("%Y-%m-%d").to_string();
        let path = dir.join(format!("{ts}-{id}.json"));
        self.atomic_write(&path, profile_json).await?;
        tracing::debug!(%project_id, "stored profile");
        Ok(())
    }

    async fn store_replay(
        &self,
        project_id: &str,
        replay_id: &str,
        data: &[u8],
    ) -> Result<(), StoreError> {
        let dir = self.project_dir(project_id).join("replays").join(replay_id);
        fs::create_dir_all(&dir).await?;
        let path = dir.join("replay.json");
        self.atomic_write(&path, data).await?;
        tracing::debug!(%project_id, %replay_id, "stored replay");
        Ok(())
    }

    async fn store_raw(
        &self,
        project_id: &str,
        item_type: &str,
        data: &[u8],
    ) -> Result<(), StoreError> {
        let dir = self.project_dir(project_id).join("raw").join(item_type);
        fs::create_dir_all(&dir).await?;
        let id = uuid::Uuid::new_v4().to_string();
        let ts = Utc::now().format("%Y-%m-%d").to_string();
        let path = dir.join(format!("{ts}-{id}.bin"));
        self.atomic_write(&path, data).await?;
        tracing::debug!(%project_id, %item_type, "stored raw item");
        Ok(())
    }

    async fn get_project_config(
        &self,
        project_id: &str,
    ) -> Result<Option<ProjectConfig>, StoreError> {
        let path = self.project_dir(project_id).join("config.json");
        if !path.exists() {
            return Ok(None);
        }
        let data = fs::read(&path).await?;
        let config = serde_json::from_slice(&data)?;
        Ok(Some(config))
    }

    async fn list_projects(&self) -> Result<Vec<ProjectConfig>, StoreError> {
        let projects_dir = self.base_dir.join("projects");
        if !projects_dir.exists() {
            return Ok(Vec::new());
        }

        let mut entries = fs::read_dir(&projects_dir).await?;
        let mut projects = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            let config_path = entry.path().join("config.json");
            if config_path.exists() {
                let data = fs::read(&config_path).await?;
                if let Ok(config) = serde_json::from_slice::<ProjectConfig>(&data) {
                    projects.push(config);
                }
            }
        }

        Ok(projects)
    }

    async fn list_events(
        &self,
        project_id: &str,
        filter: EventFilter,
    ) -> Result<Vec<EventSummary>, StoreError> {
        let events_dir = self.project_dir(project_id).join("events");
        if !events_dir.exists() {
            return Ok(Vec::new());
        }

        let limit = filter.limit.unwrap_or(100);
        let mut summaries = Vec::new();

        let mut month_entries: Vec<_> = fs::read_dir(&events_dir)
            .await?
            .entries()
            .await?
            .iter()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                Some((name, e.path()))
            })
            .collect();
        month_entries.sort_by(|a, b| b.0.cmp(&a.0)); // reverse chronological

        let mut past_cursor = filter.cursor.is_none();

        for (_month, month_path) in month_entries {
            if summaries.len() >= limit {
                break;
            }

            let mut file_entries: Vec<_> = fs::read_dir(&month_path)
                .await?
                .entries()
                .await?
                .into_iter()
                .filter_map(|e| {
                    let path = e.path();
                    if path.extension().map(|e| e == "json").unwrap_or(false) {
                        Some(path)
                    } else {
                        None
                    }
                })
                .collect();
            // Sort by file modification time (newest first) — file name has event ID
            file_entries.sort_by(|a, b| b.file_name().cmp(&a.file_name()));

            for path in file_entries {
                if summaries.len() >= limit {
                    break;
                }

                if let Ok(data) = fs::read(&path).await {
                    if let Ok(event) = serde_json::from_slice::<Event>(&data) {
                        // Cursor: skip until we pass the cursor
                        if !past_cursor {
                            if let Some(ref cursor) = filter.cursor {
                                let eid = event.event_id.map(|id| id.to_string()).unwrap_or_default();
                                if eid == *cursor {
                                    past_cursor = true;
                                }
                                continue;
                            }
                        }

                        // Apply level filter
                        if let Some(ref filter_level) = filter.level {
                            let el = event.level.map(|l| format!("{l:?}").to_lowercase());
                            let el = el.as_deref();
                            if el != Some(filter_level.as_str()) {
                                continue;
                            }
                        }

                        // Apply platform filter
                        if let Some(ref platform) = filter.platform {
                            if event.platform.as_deref() != Some(platform.as_str()) {
                                continue;
                            }
                        }

                        // Apply environment filter
                        if let Some(ref env) = filter.environment {
                            if event.environment.as_deref() != Some(env.as_str()) {
                                continue;
                            }
                        }

                        // Apply release filter
                        if let Some(ref rel) = filter.release {
                            if event.release.as_deref() != Some(rel.as_str()) {
                                continue;
                            }
                        }

                        // Apply text query
                        if let Some(ref query) = filter.query {
                            if !matches_query(&event, query) {
                                continue;
                            }
                        }

                        summaries.push(EventSummary::from_event(&event, project_id));
                    }
                }
            }
        }

        Ok(summaries)
    }

    async fn get_event(
        &self,
        project_id: &str,
        event_id: &str,
    ) -> Result<Option<Event>, StoreError> {
        let events_dir = self.project_dir(project_id).join("events");
        if !events_dir.exists() {
            return Ok(None);
        }

        let mut month_entries = fs::read_dir(&events_dir).await?;
        while let Some(entry) = month_entries.next_entry().await? {
            let path = entry.path().join(format!("{event_id}.json"));
            if path.exists() {
                let data = fs::read(&path).await?;
                let event = serde_json::from_slice(&data)?;
                return Ok(Some(event));
            }
        }

        Ok(None)
    }

    async fn get_transaction(
        &self,
        project_id: &str,
        transaction_id: &str,
    ) -> Result<Option<Event>, StoreError> {
        let tx_dir = self.project_dir(project_id).join("transactions");
        if !tx_dir.exists() {
            return Ok(None);
        }

        let mut month_entries = fs::read_dir(&tx_dir).await?;
        while let Some(entry) = month_entries.next_entry().await? {
            let path = entry.path().join(format!("{transaction_id}.json"));
            if path.exists() {
                let data = fs::read(&path).await?;
                let event = serde_json::from_slice(&data)?;
                return Ok(Some(event));
            }
        }

        Ok(None)
    }

    async fn list_transactions(
        &self,
        project_id: &str,
        filter: EventFilter,
    ) -> Result<Vec<EventSummary>, StoreError> {
        let tx_dir = self.project_dir(project_id).join("transactions");
        if !tx_dir.exists() {
            return Ok(Vec::new());
        }

        let limit = filter.limit.unwrap_or(50);
        let mut summaries = Vec::new();

        let mut month_entries: Vec<_> = fs::read_dir(&tx_dir)
            .await?
            .entries()
            .await?
            .iter()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                Some((name, e.path()))
            })
            .collect();
        month_entries.sort_by(|a, b| b.0.cmp(&a.0));

        for (_month, month_path) in month_entries {
            if summaries.len() >= limit {
                break;
            }

            let mut file_entries: Vec<_> = fs::read_dir(&month_path)
                .await?
                .entries()
                .await?
                .into_iter()
                .filter_map(|e| {
                    let path = e.path();
                    if path.extension().map(|e| e == "json").unwrap_or(false) {
                        Some(path)
                    } else {
                        None
                    }
                })
                .collect();
            file_entries.sort_by(|a, b| b.file_name().cmp(&a.file_name()));

            for path in file_entries {
                if summaries.len() >= limit {
                    break;
                }

                if let Ok(data) = fs::read(&path).await {
                    if let Ok(event) = serde_json::from_slice::<Event>(&data) {
                        if let Some(ref query) = filter.query {
                            if !matches_query(&event, query) {
                                continue;
                            }
                        }
                        summaries.push(EventSummary::from_event(&event, project_id));
                    }
                }
            }
        }

        Ok(summaries)
    }

    async fn get_project_stats(
        &self,
        project_id: &str,
    ) -> Result<ProjectStats, StoreError> {
        let project_dir = self.project_dir(project_id);
        let mut stats = ProjectStats {
            project_id: project_id.to_string(),
            total_events: 0,
            total_transactions: 0,
            total_sessions: 0,
            events_by_level: std::collections::HashMap::new(),
            recent_errors: 0,
            last_event_at: None,
        };

        // Count events
        let events_dir = project_dir.join("events");
        if events_dir.exists() {
            let mut month_entries = fs::read_dir(&events_dir).await?;
            while let Some(entry) = month_entries.next_entry().await? {
                let files: Vec<_> = fs::read_dir(entry.path())
                    .await?
                    .entries()
                    .await?
                    .into_iter()
                    .filter(|e| {
                        e.path()
                            .extension()
                            .map(|ext| ext == "json")
                            .unwrap_or(false)
                    })
                    .collect();

                stats.total_events += files.len() as u64;

                // Sample files for level breakdown and last event time
                for file in &files {
                    if let Ok(data) = fs::read(file.path()).await {
                        if let Ok(event) = serde_json::from_slice::<Event>(&data) {
                            let level_str = event
                                .level
                                .map(|l| format!("{l:?}").to_lowercase())
                                .unwrap_or_else(|| "unknown".to_string());
                            *stats.events_by_level.entry(level_str).or_insert(0) += 1;

                            if let Some(ts) = event.timestamp {
                                let ts_str = ts.to_rfc3339();
                                if stats.last_event_at.as_ref().map_or(true, |t| t < &ts_str) {
                                    stats.last_event_at = Some(ts_str);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Count transactions
        let tx_dir = project_dir.join("transactions");
        if tx_dir.exists() {
            let mut month_entries = fs::read_dir(&tx_dir).await?;
            while let Some(entry) = month_entries.next_entry().await? {
                let count = fs::read_dir(entry.path())
                    .await?
                    .entries()
                    .await?
                    .iter()
                    .filter(|e| {
                        e.path()
                            .extension()
                            .map(|ext| ext == "json")
                            .unwrap_or(false)
                    })
                    .count();
                stats.total_transactions += count as u64;
            }
        }

        // Count sessions (by file size / line count approximation)
        let sessions_dir = project_dir.join("sessions");
        if sessions_dir.exists() {
            let mut entries = fs::read_dir(&sessions_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                if let Ok(data) = fs::read(entry.path()).await {
                    stats.total_sessions += data.iter().filter(|&&b| b == b'\n').count() as u64;
                }
            }
        }

        // Recent errors (last 24h)
        stats.recent_errors = stats
            .events_by_level
            .get("error")
            .copied()
            .unwrap_or(0)
            + stats.events_by_level.get("fatal").copied().unwrap_or(0);

        Ok(stats)
    }

    async fn list_releases(
        &self,
        project_id: &str,
    ) -> Result<Vec<ReleaseSummary>, StoreError> {
        let events_dir = self.project_dir(project_id).join("events");
        if !events_dir.exists() {
            return Ok(Vec::new());
        }

        let mut releases: std::collections::HashMap<String, ReleaseSummary> =
            std::collections::HashMap::new();

        let mut month_entries = fs::read_dir(&events_dir).await?;
        while let Some(entry) = month_entries.next_entry().await? {
            let files = fs::read_dir(entry.path())
                .await?
                .entries()
                .await?;

            for file in &files {
                if file
                    .path()
                    .extension()
                    .map(|e| e == "json")
                    .unwrap_or(false)
                {
                    if let Ok(data) = fs::read(file.path()).await {
                        if let Ok(event) = serde_json::from_slice::<Event>(&data) {
                            if let Some(ref release) = event.release {
                                let entry = releases.entry(release.clone()).or_insert_with(|| {
                                    ReleaseSummary {
                                        release: release.clone(),
                                        environment: event.environment.clone(),
                                        event_count: 0,
                                        first_seen: None,
                                        last_seen: None,
                                    }
                                });
                                entry.event_count += 1;
                                if let Some(ts) = event.timestamp {
                                    let ts_str = ts.to_rfc3339();
                                    if entry.first_seen.as_ref().map_or(true, |t| t > &ts_str) {
                                        entry.first_seen = Some(ts_str.clone());
                                    }
                                    if entry.last_seen.as_ref().map_or(true, |t| t < &ts_str) {
                                        entry.last_seen = Some(ts_str);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut result: Vec<_> = releases.into_values().collect();
        result.sort_by(|a, b| b.last_seen.cmp(&a.last_seen));
        Ok(result)
    }

    async fn list_attachments(
        &self,
        project_id: &str,
        event_id: &str,
    ) -> Result<Vec<String>, StoreError> {
        let dir = self
            .project_dir(project_id)
            .join("attachments")
            .join(event_id);
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut entries = fs::read_dir(&dir).await?;
        let mut names = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            names.push(entry.file_name().to_string_lossy().to_string());
        }
        Ok(names)
    }

    async fn get_attachment(
        &self,
        project_id: &str,
        event_id: &str,
        filename: &str,
    ) -> Result<Option<Vec<u8>>, StoreError> {
        let path = self
            .project_dir(project_id)
            .join("attachments")
            .join(event_id)
            .join(filename);
        if !path.exists() {
            return Ok(None);
        }
        let data = fs::read(&path).await?;
        Ok(Some(data))
    }

    async fn delete_project(
        &self,
        project_id: &str,
    ) -> Result<(), StoreError> {
        let dir = self.project_dir(project_id);
        if dir.exists() {
            fs::remove_dir_all(&dir).await?;
        }
        self.channels.remove(project_id);
        tracing::info!(%project_id, "deleted project");
        Ok(())
    }

    async fn ensure_project(
        &self,
        project_id: &str,
        name: &str,
        public_key: &str,
    ) -> Result<ProjectConfig, StoreError> {
        let dir = self.project_dir(project_id);
        fs::create_dir_all(&dir).await?;

        let config_path = dir.join("config.json");
        if config_path.exists() {
            let data = fs::read(&config_path).await?;
            let config: ProjectConfig = serde_json::from_slice(&data)?;
            return Ok(config);
        }

        let config = ProjectConfig {
            project_id: project_id.to_string(),
            project_name: name.to_string(),
            keys: vec![ProjectKey {
                public_key: public_key.to_string(),
                secret_key: None,
            }],
        };

        let json = serde_json::to_string_pretty(&config)?;
        self.atomic_write(&config_path, json.as_bytes()).await?;

        tracing::info!(%project_id, %name, "created project");
        Ok(config)
    }

    fn subscribe_events(&self, project_id: &str) -> tokio::sync::broadcast::Receiver<String> {
        if let Some(tx) = self.channels.get(project_id) {
            tx.subscribe()
        } else {
            let (tx, rx) = tokio::sync::broadcast::channel(BROADCAST_CAPACITY);
            self.channels.insert(project_id.to_string(), tx);
            rx
        }
    }
}

// Helper for read_dir entries
trait DirEntriesExt {
    async fn entries(self) -> Result<Vec<fs::DirEntry>, std::io::Error>;
}

impl DirEntriesExt for fs::ReadDir {
    async fn entries(mut self) -> Result<Vec<fs::DirEntry>, std::io::Error> {
        let mut result = Vec::new();
        while let Some(entry) = self.next_entry().await? {
            result.push(entry);
        }
        Ok(result)
    }
}
