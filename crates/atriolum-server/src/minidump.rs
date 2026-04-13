/// Minimal multipart/form-data parser for the minidump endpoint.
///
/// This is intentionally simple — it only needs to handle the C++ SDK's
/// minidump upload format, not the full multipart spec.

/// A parsed multipart part.
#[allow(dead_code)]
pub struct MultipartPart {
    pub name: String,
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub data: Vec<u8>,
}

/// Extract the boundary string from a Content-Type header value.
///
/// Input: `multipart/form-data; boundary=----WebKitFormBoundary...`
pub fn extract_boundary(content_type: &str) -> Option<String> {
    for part in content_type.split(';') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix("boundary=") {
            return Some(rest.trim_matches('"').to_string());
        }
    }
    None
}

/// Parse multipart/form-data body into parts.
pub fn parse_multipart(body: &[u8], boundary: &str) -> Result<Vec<MultipartPart>, String> {
    let boundary_bytes = format!("--{boundary}");
    let boundary_pattern = boundary_bytes.as_bytes();

    // Split on boundary
    let mut parts = Vec::new();
    let mut pos = 0;

    // Find first boundary
    let first = find_sequence(body, boundary_pattern, pos)
        .ok_or_else(|| "no boundary found".to_string())?;
    pos = first + boundary_pattern.len();

    // Skip past the initial \r\n after boundary
    if pos < body.len() && body[pos] == b'\r' {
        pos += 2;
    } else if pos < body.len() && body[pos] == b'\n' {
        pos += 1;
    }

    loop {
        // Check for end marker (--)
        if pos + 1 < body.len() && body[pos] == b'-' && body[pos + 1] == b'-' {
            break;
        }

        // Find next boundary
        let next = match find_sequence(body, boundary_pattern, pos) {
            Some(n) => n,
            None => break,
        };

        // Content is between pos and next boundary
        // The part data ends before \r\n before the boundary
        let content_end = if next >= 2 && body[next - 2] == b'\r' && body[next - 1] == b'\n' {
            next - 2
        } else if next >= 1 && body[next - 1] == b'\n' {
            next - 1
        } else {
            next
        };

        let content = &body[pos..content_end];

        if let Some(part) = parse_part(content) {
            parts.push(part);
        }

        // Move past boundary
        pos = next + boundary_pattern.len();

        // Skip \r\n after boundary
        if pos < body.len() && body[pos] == b'\r' {
            pos += 2;
        } else if pos < body.len() && body[pos] == b'\n' {
            pos += 1;
        }
    }

    Ok(parts)
}

/// Parse a single multipart part (headers + body).
fn parse_part(content: &[u8]) -> Option<MultipartPart> {
    // Split headers from body at \r\n\r\n
    let header_end = find_sequence(content, b"\r\n\r\n", 0)?;
    let header_str = std::str::from_utf8(&content[..header_end]).ok()?;
    let body_start = header_end + 4;
    let data = content[body_start..].to_vec();

    // Parse Content-Disposition
    let mut name = String::new();
    let mut filename = None;
    let mut content_type = None;

    for line in header_str.split("\r\n") {
        let line_lower = line.to_ascii_lowercase();
        if let Some(rest) = line_lower.strip_prefix("content-disposition:") {
            let rest = rest.trim();
            for field in rest.split(';') {
                let field = field.trim();
                if let Some(v) = field.strip_prefix("name=") {
                    name = v.trim_matches('"').to_string();
                } else if let Some(v) = field.strip_prefix("filename=") {
                    filename = Some(v.trim_matches('"').to_string());
                }
            }
        } else if let Some(rest) = line_lower.strip_prefix("content-type:") {
            content_type = Some(rest.trim().to_string());
        }
    }

    if name.is_empty() {
        return None;
    }

    Some(MultipartPart {
        name,
        filename,
        content_type,
        data,
    })
}

/// Find a byte sequence in a slice, starting from `from`.
fn find_sequence(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || from + needle.len() > haystack.len() {
        return None;
    }
    for i in from..=haystack.len() - needle.len() {
        if &haystack[i..i + needle.len()] == needle {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_boundary() {
        assert_eq!(
            extract_boundary("multipart/form-data; boundary=----abc123"),
            Some("----abc123".to_string())
        );
        assert_eq!(
            extract_boundary("multipart/form-data; boundary=\"----abc\""),
            Some("----abc".to_string())
        );
        assert_eq!(extract_boundary("multipart/form-data"), None);
    }

    #[test]
    fn test_parse_multipart_minidump() {
        let body = concat!(
            "------boundary\r\n",
            "Content-Disposition: form-data; name=\"sentry\"\r\n",
            "\r\n",
            "{\"event_id\":\"abc123\"}\r\n",
            "------boundary\r\n",
            "Content-Disposition: form-data; name=\"upload_file_minidump\"; filename=\"minidump.dmp\"\r\n",
            "Content-Type: application/octet-stream\r\n",
            "\r\n",
        );
        // Build body with binary minidump data
        let mut body_bytes = body.as_bytes().to_vec();
        body_bytes.extend_from_slice(b"MDMP\x00\x01\x02\x03"); // fake minidump header
        body_bytes.extend_from_slice(b"\r\n------boundary--\r\n");

        let parts = parse_multipart(&body_bytes, "----boundary").unwrap();
        assert_eq!(parts.len(), 2);

        assert_eq!(parts[0].name, "sentry");
        assert_eq!(
            std::str::from_utf8(&parts[0].data).unwrap(),
            "{\"event_id\":\"abc123\"}"
        );

        assert_eq!(parts[1].name, "upload_file_minidump");
        assert_eq!(parts[1].filename.as_deref(), Some("minidump.dmp"));
        assert_eq!(&parts[1].data[..4], b"MDMP");
    }

    #[test]
    fn test_parse_multipart_empty() {
        let body = "--boundary--\r\n";
        let parts = parse_multipart(body.as_bytes(), "boundary").unwrap();
        assert!(parts.is_empty());
    }
}
