use std::process::Command;
use std::{fs, thread, time::Duration};

/// Helper to find an available port.
fn find_port() -> u16 {
    use std::net::TcpListener;
    TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Integration test: start server, send events via curl, verify filesystem.
#[test]
fn test_server_store_endpoint() {
    let port = find_port();
    let tmpdir = tempfile::TempDir::new().unwrap();
    let data_dir = tmpdir.path().to_str().unwrap();
    let base = format!("http://127.0.0.1:{port}");

    // Start server in background
    let mut server = Command::new("cargo")
        .args([
            "run",
            "-p",
            "atriolum-server",
            "--",
            "--port",
            &port.to_string(),
            "--data-dir",
            data_dir,
        ])
        .env("RUST_LOG", "off")
        .spawn()
        .expect("failed to start server");

    // Wait for server to start
    thread::sleep(Duration::from_secs(3));

    // Test health endpoint
    let health = Command::new("curl")
        .args(["-s", &format!("{base}/api/health")])
        .output()
        .expect("curl failed");
    assert!(health.status.success());
    assert!(String::from_utf8_lossy(&health.stdout).contains("ok"));

    // Test store endpoint
    let store_result = Command::new("curl")
        .args([
            "-s",
            "-X",
            "POST",
            &format!("{base}/api/1/store/"),
            "-H",
            "X-Sentry-Auth: Sentry sentry_version=7, sentry_key=testkey",
            "-H",
            "Content-Type: application/json",
            "-d",
            r#"{"event_id":"fc6d8c0c43fc4630ad850ee518f1b9d0","message":"integration test","level":"error","platform":"python","timestamp":"2026-04-13T10:30:00Z"}"#,
        ])
        .output()
        .expect("curl failed");
    let body = String::from_utf8_lossy(&store_result.stdout);
    assert!(body.contains("fc6d8c0c"), "unexpected response: {body}");

    // Verify file on disk
    let event_path = format!("{data_dir}/projects/1/events/2026-04/fc6d8c0c-43fc-4630-ad85-0ee518f1b9d0.json");
    assert!(fs::metadata(&event_path).is_ok(), "event file not found: {event_path}");

    // Test auth failure
    let auth_fail = Command::new("curl")
        .args([
            "-s",
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
            "-X",
            "POST",
            &format!("{base}/api/1/store/"),
            "-H",
            "Content-Type: application/json",
            "-d",
            r#"{"event_id":"abc","message":"no auth"}"#,
        ])
        .output()
        .expect("curl failed");
    let status = String::from_utf8_lossy(&auth_fail.stdout);
    assert_eq!(status, "403", "expected 403 for missing auth, got: {status}");

    // Test 404
    let not_found = Command::new("curl")
        .args([
            "-s",
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
            &format!("{base}/nonexistent"),
        ])
        .output()
        .expect("curl failed");
    let status = String::from_utf8_lossy(&not_found.stdout);
    assert_eq!(status, "404", "expected 404, got: {status}");

    // Cleanup
    let _ = server.kill();
}

#[test]
fn test_server_envelope_endpoint() {
    let port = find_port();
    let tmpdir = tempfile::TempDir::new().unwrap();
    let data_dir = tmpdir.path().to_str().unwrap();

    let mut server = Command::new("cargo")
        .args([
            "run",
            "-p",
            "atriolum-server",
            "--",
            "--port",
            &port.to_string(),
            "--data-dir",
            data_dir,
        ])
        .env("RUST_LOG", "off")
        .spawn()
        .expect("failed to start server");

    thread::sleep(Duration::from_secs(3));

    // Create envelope data
    let envelope = format!(
        "{{\"event_id\":\"a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4\"}}\n{{\"type\":\"event\",\"length\":{}}}\n{{\"message\":\"envelope test\",\"level\":\"warning\",\"platform\":\"javascript\"}}\n",
        r#"{"message":"envelope test","level":"warning","platform":"javascript"}"#.len()
    );

    // Write envelope to temp file
    let envelope_file = tmpdir.path().join("envelope.txt");
    fs::write(&envelope_file, &envelope).unwrap();

    let result = Command::new("curl")
        .args([
            "-s",
            "-X",
            "POST",
            &format!("http://127.0.0.1:{port}/api/2/envelope/"),
            "-H",
            "X-Sentry-Auth: Sentry sentry_version=7, sentry_key=testkey",
            "-H",
            "Content-Type: application/x-sentry-envelope",
            "--data-binary",
            &format!("@{}", envelope_file.display()),
        ])
        .output()
        .expect("curl failed");

    let body = String::from_utf8_lossy(&result.stdout);
    assert!(body.contains("a1b2c3d4"), "unexpected response: {body}");

    // Verify file exists
    let events_dir = format!("{data_dir}/projects/2/events/2026-04");
    let found = fs::read_dir(&events_dir)
        .expect("events dir not found")
        .count();
    assert_eq!(found, 1, "expected 1 event file");

    let _ = server.kill();
}

#[test]
fn test_server_minidump_endpoint() {
    let port = find_port();
    let tmpdir = tempfile::TempDir::new().unwrap();
    let data_dir = tmpdir.path().to_str().unwrap();

    let mut server = Command::new("cargo")
        .args([
            "run",
            "-p",
            "atriolum-server",
            "--",
            "--port",
            &port.to_string(),
            "--data-dir",
            data_dir,
        ])
        .env("RUST_LOG", "off")
        .spawn()
        .expect("failed to start server");

    thread::sleep(Duration::from_secs(3));

    // Build multipart body
    let boundary = "----TestBoundary123";
    let mut body = Vec::new();
    body.extend_from_slice(format!("------TestBoundary123\r\n").as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"sentry\"\r\n\r\n");
    body.extend_from_slice(b"{\"event_id\":\"deadbeef12345678deadbeef12345678\",\"message\":\"crash\"}\r\n");
    body.extend_from_slice(format!("------TestBoundary123\r\n").as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"upload_file_minidump\"; filename=\"minidump.dmp\"\r\n");
    body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
    body.extend_from_slice(b"MDMP\x00\x01\x02\x03\x04\x05\x06\x07");
    body.extend_from_slice(b"\r\n------TestBoundary123--\r\n");

    let body_file = tmpdir.path().join("minidump_body.bin");
    fs::write(&body_file, &body).unwrap();

    let result = Command::new("curl")
        .args([
            "-s",
            "-X",
            "POST",
            &format!("http://127.0.0.1:{port}/api/3/minidump/"),
            "-H",
            "X-Sentry-Auth: Sentry sentry_version=7, sentry_key=testkey",
            "-H",
            &format!("Content-Type: multipart/form-data; boundary={boundary}"),
            "--data-binary",
            &format!("@{}", body_file.display()),
        ])
        .output()
        .expect("curl failed");

    let resp = String::from_utf8_lossy(&result.stdout);
    assert!(resp.contains("deadbeef"), "unexpected minidump response: {resp}");

    // Verify event file
    let events_dir = format!("{data_dir}/projects/3/events");
    let found = fs::read_dir(&events_dir)
        .expect("events dir not found")
        .flatten()
        .flat_map(|d| fs::read_dir(d.path()).unwrap().flatten())
        .count();
    assert_eq!(found, 1, "expected 1 event file from minidump");

    // Verify minidump attachment
    let attachments_dir = format!("{data_dir}/projects/3/attachments");
    let found = fs::read_dir(&attachments_dir)
        .expect("attachments dir not found")
        .flatten()
        .flat_map(|d| fs::read_dir(d.path()).unwrap().flatten())
        .count();
    assert!(found >= 1, "expected at least 1 attachment from minidump");

    let _ = server.kill();
}

#[test]
fn test_server_rate_limit_headers() {
    let port = find_port();
    let tmpdir = tempfile::TempDir::new().unwrap();
    let data_dir = tmpdir.path().to_str().unwrap();

    let mut server = Command::new("cargo")
        .args([
            "run",
            "-p",
            "atriolum-server",
            "--",
            "--port",
            &port.to_string(),
            "--data-dir",
            data_dir,
        ])
        .env("RUST_LOG", "off")
        .spawn()
        .expect("failed to start server");

    thread::sleep(Duration::from_secs(3));

    // Check that rate limit header is present on success
    let result = Command::new("curl")
        .args([
            "-s",
            "-D",
            "-",
            "-X",
            "POST",
            &format!("http://127.0.0.1:{port}/api/4/envelope/"),
            "-H",
            "X-Sentry-Auth: Sentry sentry_version=7, sentry_key=testkey",
            "-H",
            "Content-Type: application/x-sentry-envelope",
            "-d",
            "{}\n{\"type\":\"event\"}\n{}\n",
        ])
        .output()
        .expect("curl failed");

    let headers = String::from_utf8_lossy(&result.stdout);
    assert!(
        headers.to_lowercase().contains("x-sentry-rate-limits"),
        "missing x-sentry-rate-limits header: {headers}"
    );

    let _ = server.kill();
}

#[test]
fn test_management_api() {
    let port = find_port();
    let tmpdir = tempfile::TempDir::new().unwrap();
    let data_dir = tmpdir.path().to_str().unwrap();

    let mut server = Command::new("cargo")
        .args([
            "run",
            "-p",
            "atriolum-server",
            "--",
            "--port",
            &port.to_string(),
            "--data-dir",
            data_dir,
        ])
        .env("RUST_LOG", "off")
        .spawn()
        .expect("failed to start server");

    thread::sleep(Duration::from_secs(3));
    let base = format!("http://127.0.0.1:{port}");

    // 1. GET /api/projects/ — empty initially
    let result = Command::new("curl")
        .args(["-s", &format!("{base}/api/projects/")])
        .output()
        .unwrap();
    let body = String::from_utf8_lossy(&result.stdout);
    assert!(body == "[]", "expected empty projects, got: {body}");

    // 2. POST /api/projects/ — create project
    let result = Command::new("curl")
        .args([
            "-s",
            "-X",
            "POST",
            &format!("{base}/api/projects/"),
            "-H",
            "Content-Type: application/json",
            "-d",
            r#"{"name":"test-project","project_id":"42","public_key":"mykey123"}"#,
        ])
        .output()
        .unwrap();
    let body = String::from_utf8_lossy(&result.stdout);
    assert!(body.contains("42"), "project create response: {body}");
    assert!(body.contains("mykey123"), "project create response: {body}");

    // 3. GET /api/projects/ — now has one project
    let result = Command::new("curl")
        .args(["-s", &format!("{base}/api/projects/")])
        .output()
        .unwrap();
    let body = String::from_utf8_lossy(&result.stdout);
    assert!(body.contains("test-project"), "projects list: {body}");

    // 4. GET /api/projects/42/ — get single project
    let result = Command::new("curl")
        .args(["-s", &format!("{base}/api/projects/42/")])
        .output()
        .unwrap();
    let body = String::from_utf8_lossy(&result.stdout);
    assert!(body.contains("test-project"), "project detail: {body}");

    // 5. POST event via SDK endpoint
    let _ = Command::new("curl")
        .args([
            "-s",
            "-X",
            "POST",
            &format!("{base}/api/42/envelope/"),
            "-H",
            "X-Sentry-Auth: Sentry sentry_version=7, sentry_key=mykey123",
            "-H",
            "Content-Type: application/x-sentry-envelope",
            "-d",
            &format!(
                "{{\"event_id\":\"fc6d8c0c43fc4630ad850ee518f1b9d0\"}}\n{{\"type\":\"event\",\"length\":{}}}\n{{\"message\":\"test error\",\"level\":\"error\",\"platform\":\"python\",\"environment\":\"testing\"}}\n",
                r#"{"message":"test error","level":"error","platform":"python","environment":"testing"}"#.len()
            ),
        ])
        .output()
        .unwrap();

    // 6. GET /api/projects/42/events/ — list events
    let result = Command::new("curl")
        .args(["-s", &format!("{base}/api/projects/42/events/")])
        .output()
        .unwrap();
    let body = String::from_utf8_lossy(&result.stdout);
    assert!(body.contains("fc6d8c0c"), "events list: {body}");

    // 7. GET /api/projects/42/events/?level=error — filtered
    let result = Command::new("curl")
        .args(["-s", &format!("{base}/api/projects/42/events/?level=error")])
        .output()
        .unwrap();
    let body = String::from_utf8_lossy(&result.stdout);
    assert!(body.contains("fc6d8c0c"), "filtered events: {body}");

    // 8. GET /api/projects/42/stats/
    let result = Command::new("curl")
        .args(["-s", &format!("{base}/api/projects/42/stats/")])
        .output()
        .unwrap();
    let body = String::from_utf8_lossy(&result.stdout);
    assert!(body.contains("total_events"), "stats: {body}");

    // 9. GET /api/projects/42/releases/
    let result = Command::new("curl")
        .args(["-s", &format!("{base}/api/projects/42/releases/")])
        .output()
        .unwrap();
    let body = String::from_utf8_lossy(&result.stdout);
    assert!(body == "[]", "releases: {body}");

    // 10. DELETE /api/projects/42/
    let result = Command::new("curl")
        .args([
            "-s",
            "-X",
            "DELETE",
            &format!("{base}/api/projects/42/"),
        ])
        .output()
        .unwrap();
    let body = String::from_utf8_lossy(&result.stdout);
    assert!(body.contains("deleted"), "delete response: {body}");

    // 11. Verify project is gone
    let result = Command::new("curl")
        .args(["-s", &format!("{base}/api/projects/42/")])
        .output()
        .unwrap();
    let body = String::from_utf8_lossy(&result.stdout);
    assert!(body.contains("not found"), "project should be deleted: {body}");

    let _ = server.kill();
}
