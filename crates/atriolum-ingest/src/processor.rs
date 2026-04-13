use atriolum_protocol::{Envelope, Event, KnownItemType};
use atriolum_store::Store;
use tracing;

use crate::error::IngestError;

/// Result of processing an envelope.
#[derive(Debug)]
pub struct ProcessedResult {
    pub event_id: Option<String>,
    pub items_processed: usize,
    pub items_skipped: usize,
}

/// Main ingest processor that routes envelope items to storage.
pub struct IngestProcessor<S: Store> {
    store: S,
}

impl<S: Store> IngestProcessor<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Process a parsed envelope.
    pub async fn process_envelope(
        &self,
        project_id: &str,
        envelope: Envelope,
    ) -> Result<ProcessedResult, IngestError> {
        let event_id = envelope
            .header
            .event_id
            .map(|id| id.to_string());

        let mut items_processed = 0;
        let mut items_skipped = 0;

        for item in &envelope.items {
            match KnownItemType::from_str(&item.header.item_type) {
                Some(KnownItemType::Event) => {
                    let event: Event = serde_json::from_slice(&item.payload)?;
                    self.store
                        .store_event(project_id, &event, &item.payload)
                        .await?;
                    items_processed += 1;
                }
                Some(KnownItemType::Transaction) => {
                    let event: Event = serde_json::from_slice(&item.payload)?;
                    self.store
                        .store_transaction(project_id, &event, &item.payload)
                        .await?;
                    items_processed += 1;
                }
                Some(KnownItemType::Session) | Some(KnownItemType::Sessions) => {
                    self.store
                        .store_session(project_id, &item.payload)
                        .await?;
                    items_processed += 1;
                }
                Some(KnownItemType::Attachment) => {
                    let eid = event_id.as_deref().unwrap_or("unknown");
                    let filename = item
                        .header
                        .filename
                        .as_deref()
                        .unwrap_or("unnamed_attachment");
                    self.store
                        .store_attachment(project_id, eid, filename, &item.payload)
                        .await?;
                    items_processed += 1;
                }
                Some(KnownItemType::ClientReport) => {
                    self.store
                        .store_client_report(project_id, &item.payload)
                        .await?;
                    items_processed += 1;
                }
                Some(KnownItemType::UserReport) => {
                    // Store user reports separately
                    self.store
                        .store_client_report(project_id, &item.payload)
                        .await?;
                    items_processed += 1;
                }
                Some(KnownItemType::Log) => {
                    self.store
                        .store_logs(project_id, &item.payload)
                        .await?;
                    items_processed += 1;
                }
                Some(KnownItemType::Span) => {
                    self.store
                        .store_span(project_id, &item.payload)
                        .await?;
                    items_processed += 1;
                }
                Some(KnownItemType::CheckIn) => {
                    self.store
                        .store_check_in(project_id, &item.payload)
                        .await?;
                    items_processed += 1;
                }
                Some(KnownItemType::Profile) | Some(KnownItemType::ProfileChunk) => {
                    self.store
                        .store_profile(project_id, &item.payload)
                        .await?;
                    items_processed += 1;
                }
                Some(KnownItemType::ReplayEvent) | Some(KnownItemType::ReplayRecording) => {
                    let rid = event_id.as_deref().unwrap_or("unknown");
                    self.store
                        .store_replay(project_id, rid, &item.payload)
                        .await?;
                    items_processed += 1;
                }
                Some(KnownItemType::Statsd) | Some(KnownItemType::MetricMeta) => {
                    // Metrics: store as raw for now
                    self.store
                        .store_raw(project_id, &item.header.item_type, &item.payload)
                        .await?;
                    items_processed += 1;
                }
                None => {
                    tracing::warn!(
                        item_type = %item.header.item_type,
                        "skipping unknown item type"
                    );
                    items_skipped += 1;
                }
            }
        }

        tracing::info!(
            %project_id,
            ?event_id,
            items_processed,
            items_skipped,
            "processed envelope"
        );

        Ok(ProcessedResult {
            event_id,
            items_processed,
            items_skipped,
        })
    }
}

/// Wrap a raw event JSON (from the legacy `/store/` endpoint) as an Envelope.
pub fn wrap_event_as_envelope(event_json: &[u8]) -> Result<Envelope, IngestError> {
    let event: Event = serde_json::from_slice(event_json)?;
    let event_id = event.event_id;

    // Build a synthetic envelope
    let mut items = Vec::new();
    items.push(atriolum_protocol::EnvelopeItem {
        header: atriolum_protocol::ItemHeader {
            item_type: "event".to_string(),
            length: Some(event_json.len()),
            content_type: Some("application/json".to_string()),
            filename: None,
            attachment_type: None,
        },
        payload: bytes::Bytes::copy_from_slice(event_json),
    });

    Ok(Envelope {
        header: atriolum_protocol::EnvelopeHeader {
            event_id,
            dsn: None,
            sdk: event.sdk.clone(),
            sent_at: None,
        },
        items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_event_as_envelope() {
        let json = r#"{"event_id":"fc6d8c0c43fc4630ad850ee518f1b9d0","message":"hello","level":"error"}"#;
        let envelope = wrap_event_as_envelope(json.as_bytes()).unwrap();
        assert_eq!(envelope.items.len(), 1);
        let id = envelope.header.event_id.unwrap();
        assert_eq!(id.to_string().replace("-", ""), "fc6d8c0c43fc4630ad850ee518f1b9d0");
    }
}
