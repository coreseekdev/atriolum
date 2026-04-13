use atriolum_protocol::{parse_dsn, parse_sentry_auth, ProjectConfig, SentryAuth};

use crate::error::IngestError;

/// Validate authentication from multiple possible sources.
///
/// Checks (in order):
/// 1. `X-Sentry-Auth` header
/// 2. Query parameters
/// 3. DSN in envelope header
///
/// Returns the validated SentryAuth or an error.
pub fn validate_auth(
    auth_header: Option<&str>,
    query_string: &str,
    envelope_dsn: Option<&str>,
    project_config: &ProjectConfig,
) -> Result<SentryAuth, IngestError> {
    // Try X-Sentry-Auth header first
    let mut auth = if let Some(header) = auth_header {
        Some(parse_sentry_auth(header).map_err(|e| {
            IngestError::AuthFailed(format!("invalid auth header: {e}"))
        })?)
    } else {
        None
    };

    // Try query string fallback
    if auth.is_none() && !query_string.is_empty() {
        let mut sentry_key = None;
        let mut sentry_version = None;

        for pair in query_string.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                match k {
                    "sentry_key" => sentry_key = Some(v.to_string()),
                    "sentry_version" => sentry_version = Some(v.to_string()),
                    _ => {}
                }
            }
        }

        if let (Some(key), Some(ver)) = (sentry_key, sentry_version) {
            auth = Some(SentryAuth {
                sentry_key: key,
                sentry_version: ver.parse().unwrap_or(0),
                sentry_client: None,
                sentry_secret: None,
            });
        }
    }

    // Try envelope DSN
    if auth.is_none() {
        if let Some(dsn_str) = envelope_dsn {
            let dsn = parse_dsn(dsn_str).map_err(|e| {
                IngestError::AuthFailed(format!("invalid DSN in envelope: {e}"))
            })?;
            auth = Some(SentryAuth {
                sentry_key: dsn.public_key,
                sentry_version: 7,
                sentry_client: None,
                sentry_secret: dsn.secret_key,
            });
        }
    }

    let auth = auth.ok_or_else(|| {
        IngestError::AuthFailed("no authentication provided".into())
    })?;

    // Validate version
    if auth.sentry_version != 7 {
        return Err(IngestError::AuthFailed(format!(
            "unsupported protocol version: {}",
            auth.sentry_version
        )));
    }

    // Validate key matches project
    let key_matches = project_config
        .keys
        .iter()
        .any(|k| k.public_key == auth.sentry_key);

    if !key_matches {
        return Err(IngestError::AuthFailed(
            "sentry_key does not match any project key".into(),
        ));
    }

    Ok(auth)
}

#[cfg(test)]
mod tests {
    use super::*;
    use atriolum_protocol::ProjectKey;

    fn test_config() -> ProjectConfig {
        ProjectConfig {
            project_id: "1".into(),
            project_name: "test".into(),
            keys: vec![ProjectKey {
                public_key: "testkey".into(),
                secret_key: None,
            }],
        }
    }

    #[test]
    fn test_validate_auth_valid_header() {
        let config = test_config();
        let auth = validate_auth(
            Some("Sentry sentry_version=7, sentry_key=testkey"),
            "",
            None,
            &config,
        )
        .unwrap();
        assert_eq!(auth.sentry_key, "testkey");
    }

    #[test]
    fn test_validate_auth_missing() {
        let config = test_config();
        let result = validate_auth(None, "", None, &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_auth_wrong_key() {
        let config = test_config();
        let result = validate_auth(
            Some("Sentry sentry_version=7, sentry_key=wrongkey"),
            "",
            None,
            &config,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_auth_via_query() {
        let config = test_config();
        let auth = validate_auth(None, "sentry_key=testkey&sentry_version=7", None, &config).unwrap();
        assert_eq!(auth.sentry_key, "testkey");
    }

    #[test]
    fn test_validate_auth_wrong_version() {
        let config = test_config();
        let result = validate_auth(
            Some("Sentry sentry_version=6, sentry_key=testkey"),
            "",
            None,
            &config,
        );
        assert!(result.is_err());
    }
}
