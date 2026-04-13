use bytes::Bytes;
use serde::Deserialize;
use uuid::Uuid;

use crate::error::ProtocolError;
use crate::types::SdkInfo;

/// Parsed envelope header (first line of envelope).
#[derive(Debug, Clone, Deserialize)]
pub struct EnvelopeHeader {
    #[serde(default)]
    pub event_id: Option<Uuid>,
    #[serde(default)]
    pub dsn: Option<String>,
    #[serde(default)]
    pub sdk: Option<SdkInfo>,
    #[serde(default)]
    pub sent_at: Option<String>,
}

/// Per-item metadata in an envelope.
#[derive(Debug, Clone, Deserialize)]
pub struct ItemHeader {
    #[serde(rename = "type")]
    pub item_type: String,
    #[serde(default)]
    pub length: Option<usize>,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub attachment_type: Option<String>,
}

/// A single item within an envelope.
#[derive(Debug, Clone)]
pub struct EnvelopeItem {
    pub header: ItemHeader,
    pub payload: Bytes,
}

/// A parsed Sentry envelope.
#[derive(Debug, Clone)]
pub struct Envelope {
    pub header: EnvelopeHeader,
    pub items: Vec<EnvelopeItem>,
}

/// Parse a Sentry envelope from raw bytes.
///
/// Envelope format: newline-delimited items.
/// First line = envelope header JSON.
/// Then pairs of: item header JSON line, payload bytes.
pub fn parse_envelope(data: &[u8]) -> Result<Envelope, ProtocolError> {
    // Work entirely at the byte level to handle binary payloads correctly
    let mut pos = 0;

    // Find the first newline (envelope header)
    let header_end = data
        .iter()
        .position(|&b| b == b'\n')
        .unwrap_or(data.len());
    let header_bytes = &data[..header_end];
    let header: EnvelopeHeader = serde_json::from_slice(header_bytes).map_err(|e| {
        ProtocolError::InvalidEnvelope(format!("invalid envelope header: {e}"))
    })?;
    pos = header_end + 1; // skip past \n

    let mut items = Vec::new();

    while pos < data.len() {
        // Skip any leading empty lines (trailing newlines)
        if data[pos] == b'\n' {
            pos += 1;
            continue;
        }

        // Find item header line
        let header_end = data[pos..]
            .iter()
            .position(|&b| b == b'\n')
            .ok_or_else(|| {
                ProtocolError::InvalidEnvelope("unexpected end of data in item header".into())
            })?;

        let item_header_bytes = &data[pos..pos + header_end];
        let item_header: ItemHeader = serde_json::from_slice(item_header_bytes).map_err(|e| {
            ProtocolError::InvalidEnvelope(format!("invalid item header: {e}"))
        })?;

        pos += header_end + 1; // skip past item header + \n

        // Read payload based on length field
        if let Some(len) = item_header.length {
            if pos + len > data.len() {
                return Err(ProtocolError::InvalidEnvelope(format!(
                    "item payload truncated: expected {len} bytes, got {}",
                    data.len() - pos
                )));
            }
            let payload = Bytes::copy_from_slice(&data[pos..pos + len]);
            pos += len;
            // Skip trailing \n after explicit-length payload
            if pos < data.len() && data[pos] == b'\n' {
                pos += 1;
            }
            items.push(EnvelopeItem {
                header: item_header,
                payload,
            });
        } else {
            // No explicit length: read until next \n
            let payload_end = data[pos..]
                .iter()
                .position(|&b| b == b'\n')
                .unwrap_or(data.len() - pos);
            let payload = Bytes::copy_from_slice(&data[pos..pos + payload_end]);
            pos += payload_end;
            // Skip the \n delimiter
            if pos < data.len() && data[pos] == b'\n' {
                pos += 1;
            }
            items.push(EnvelopeItem {
                header: item_header,
                payload,
            });
        }
    }

    Ok(Envelope { header, items })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_envelope_event_item() {
        let data = concat!(
            "{\"event_id\":\"9ec79c33ec9942ab8353589fcb2e04dc\",\"dsn\":\"https://key@sentry.io/42\"}\n",
            "{\"type\":\"event\",\"length\":41}\n",
            "{\"message\":\"hello world\",\"level\":\"error\"}\n"
        );
        let envelope = parse_envelope(data.as_bytes()).unwrap();
        let id = envelope.header.event_id.unwrap();
        assert_eq!(id.to_string().replace("-", ""), "9ec79c33ec9942ab8353589fcb2e04dc");
        assert_eq!(envelope.items.len(), 1);
        assert_eq!(envelope.items[0].header.item_type, "event");
        assert_eq!(envelope.items[0].header.length, Some(41));
    }

    #[test]
    fn test_parse_envelope_implicit_length() {
        let data = concat!(
            "{\"event_id\":\"a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4\"}\n",
            "{\"type\":\"event\"}\n",
            "{\"message\":\"test\"}\n"
        );
        let envelope = parse_envelope(data.as_bytes()).unwrap();
        assert_eq!(envelope.items.len(), 1);
        assert_eq!(envelope.items[0].payload.as_ref(), b"{\"message\":\"test\"}");
    }

    #[test]
    fn test_parse_envelope_two_items() {
        let data = concat!(
            "{\"event_id\":\"a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4\"}\n",
            "{\"type\":\"event\",\"length\":16}\n",
            "{\"message\":\"hi\"}\n",  // 16 bytes
            "{\"type\":\"attachment\",\"length\":5}\n",
            "hello\n"
        );
        let envelope = parse_envelope(data.as_bytes()).unwrap();
        assert_eq!(envelope.items.len(), 2);
        assert_eq!(envelope.items[0].header.item_type, "event");
        assert_eq!(envelope.items[1].header.item_type, "attachment");
        assert_eq!(envelope.items[1].payload.as_ref(), b"hello");
    }

    #[test]
    fn test_parse_envelope_empty_header() {
        let data = concat!("{}\n", "{\"type\":\"event\"}\n", "{\"message\":\"x\"}\n");
        let envelope = parse_envelope(data.as_bytes()).unwrap();
        assert!(envelope.header.event_id.is_none());
        assert_eq!(envelope.items.len(), 1);
    }

    #[test]
    fn test_parse_envelope_invalid_header() {
        let data = "not json\n{\"type\":\"event\"}\n{}\n";
        assert!(parse_envelope(data.as_bytes()).is_err());
    }
}
