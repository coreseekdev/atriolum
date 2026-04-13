use crate::error::ProtocolError;

/// Parsed from `X-Sentry-Auth` header or query string.
#[derive(Debug, Clone)]
pub struct SentryAuth {
    pub sentry_key: String,
    pub sentry_version: u32,
    pub sentry_client: Option<String>,
    pub sentry_secret: Option<String>,
}

/// Parsed from a DSN string.
#[derive(Debug, Clone)]
pub struct DsnInfo {
    pub scheme: String,
    pub public_key: String,
    pub secret_key: Option<String>,
    pub host: String,
    pub path: String,
    pub project_id: String,
}

/// Parse `X-Sentry-Auth` header value.
///
/// Format: `Sentry sentry_version=7, sentry_key=..., sentry_client=...`
pub fn parse_sentry_auth(header_value: &str) -> Result<SentryAuth, ProtocolError> {
    let value = header_value
        .strip_prefix("Sentry ")
        .ok_or_else(|| ProtocolError::InvalidAuth("missing 'Sentry ' prefix".into()))?;

    let mut sentry_key = None;
    let mut sentry_version = None;
    let mut sentry_client = None;
    let mut sentry_secret = None;

    for pair in value.split(',') {
        let pair = pair.trim();
        if let Some((k, v)) = pair.split_once('=') {
            match k.trim() {
                "sentry_key" => sentry_key = Some(v.trim().to_string()),
                "sentry_version" => {
                    sentry_version = Some(v.trim().parse::<u32>().map_err(|_| {
                        ProtocolError::InvalidAuth(format!("invalid sentry_version: {v}"))
                    })?)
                }
                "sentry_client" => sentry_client = Some(v.trim().to_string()),
                "sentry_secret" => sentry_secret = Some(v.trim().to_string()),
                _ => {}
            }
        }
    }

    Ok(SentryAuth {
        sentry_key: sentry_key
            .ok_or_else(|| ProtocolError::InvalidAuth("missing sentry_key".into()))?,
        sentry_version: sentry_version
            .ok_or_else(|| ProtocolError::InvalidAuth("missing sentry_version".into()))?,
        sentry_client,
        sentry_secret,
    })
}

/// Parse a Sentry DSN string.
///
/// Format: `{PROTOCOL}://{PUBLIC_KEY}:{SECRET_KEY}@{HOST}{PATH}/{PROJECT_ID}`
/// Example: `https://public@sentry.example.com/1`
pub fn parse_dsn(dsn: &str) -> Result<DsnInfo, ProtocolError> {
    let url = url::Url::parse(dsn).map_err(|e| ProtocolError::InvalidDsn(e.to_string()))?;

    let scheme = url.scheme().to_string();
    let host = url
        .host_str()
        .ok_or_else(|| ProtocolError::InvalidDsn("missing host".into()))?
        .to_string();

    let username = url.username();
    if username.is_empty() {
        return Err(ProtocolError::InvalidDsn("missing public key".into()));
    }
    let public_key = username.to_string();
    let secret_key = url.password().map(|s| s.to_string());

    let path = url.path().trim_end_matches('/').to_string();

    let project_id = path
        .rsplit('/')
        .next()
        .ok_or_else(|| ProtocolError::InvalidDsn("missing project_id".into()))?
        .to_string();

    Ok(DsnInfo {
        scheme,
        public_key,
        secret_key,
        host,
        path,
        project_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sentry_auth_valid() {
        let auth = parse_sentry_auth(
            "Sentry sentry_version=7, sentry_key=abc123, sentry_client=sentry.python/1.45.0",
        )
        .unwrap();
        assert_eq!(auth.sentry_key, "abc123");
        assert_eq!(auth.sentry_version, 7);
        assert_eq!(
            auth.sentry_client.as_deref(),
            Some("sentry.python/1.45.0")
        );
        assert!(auth.sentry_secret.is_none());
    }

    #[test]
    fn test_parse_sentry_auth_with_secret() {
        let auth = parse_sentry_auth(
            "Sentry sentry_version=7, sentry_key=abc, sentry_secret=def",
        )
        .unwrap();
        assert_eq!(auth.sentry_secret.as_deref(), Some("def"));
    }

    #[test]
    fn test_parse_sentry_auth_missing_key() {
        let result = parse_sentry_auth("Sentry sentry_version=7");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_sentry_auth_missing_prefix() {
        let result = parse_sentry_auth("sentry_version=7, sentry_key=abc");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_dsn_full() {
        let dsn = parse_dsn("https://public:secret@sentry.example.com/path/42").unwrap();
        assert_eq!(dsn.scheme, "https");
        assert_eq!(dsn.public_key, "public");
        assert_eq!(dsn.secret_key.as_deref(), Some("secret"));
        assert_eq!(dsn.host, "sentry.example.com");
        assert_eq!(dsn.project_id, "42");
    }

    #[test]
    fn test_parse_dsn_minimal() {
        let dsn = parse_dsn("https://abc123@sentry.io/1").unwrap();
        assert_eq!(dsn.public_key, "abc123");
        assert!(dsn.secret_key.is_none());
        assert_eq!(dsn.project_id, "1");
    }

    #[test]
    fn test_parse_dsn_invalid() {
        assert!(parse_dsn("not-a-url").is_err());
    }
}
