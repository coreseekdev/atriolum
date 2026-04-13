use async_trait::async_trait;
use atriolum_protocol::{Event, EventSummary, ProjectConfig, ProjectKey};
use chrono::Utc;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing;

use crate::error::StoreError;
use crate::query::EventFilter;
use crate::store::Store;

pub struct FilesystemStore {
    base_dir: PathBuf,
}

impl FilesystemStore {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
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
}

// We need Clone for sharing across handlers.
impl Clone for FilesystemStore {
    fn clone(&self) -> Self {
        Self {
            base_dir: self.base_dir.clone(),
        }
    }
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

        // Append to jsonl file
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

        // Read month directories in reverse chronological order
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
        month_entries.sort_by(|a, b| b.0.cmp(&a.0)); // reverse order

        for (_month, month_path) in month_entries {
            if summaries.len() >= limit {
                break;
            }

            let mut file_entries = fs::read_dir(&month_path).await?;
            while let Some(entry) = file_entries.next_entry().await? {
                if summaries.len() >= limit {
                    break;
                }

                let path = entry.path();
                if path.extension().map(|e| e == "json").unwrap_or(false) {
                    if let Ok(data) = fs::read(&path).await {
                        if let Ok(event) = serde_json::from_slice::<Event>(&data) {
                            // Apply level filter
                            if let Some(ref filter_level) = filter.level {
                                if event.level != Some(*filter_level) {
                                    continue;
                                }
                            }

                            summaries.push(EventSummary::from_event(&event, project_id));
                        }
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

        // Search across month directories
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

        // Create new project config
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
}

// Helper for read_dir entries
use tokio::io::AsyncWriteExt;

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
