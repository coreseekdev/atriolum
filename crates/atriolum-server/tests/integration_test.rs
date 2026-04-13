use std::process::Command;
use std::{fs, thread, time::Duration};

/// Integration test: start server, send events via curl, verify filesystem.
#[test]
fn test_server_store_endpoint() {
    let tmpdir = tempfile::TempDir::new().unwrap();
    let data_dir = tmpdir.path().to_str().unwrap();

    // Start server in background
    let mut server = Command::new("cargo")
        .args([
            "run",
            "-p",
            "atriolum-server",
            "--",
            "--port",
            "18765",
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
        .args(["-s", "http://127.0.0.1:18765/api/health"])
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
            "http://127.0.0.1:18765/api/1/store/",
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
            "http://127.0.0.1:18765/api/1/store/",
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
            "http://127.0.0.1:18765/nonexistent",
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
    let tmpdir = tempfile::TempDir::new().unwrap();
    let data_dir = tmpdir.path().to_str().unwrap();

    let mut server = Command::new("cargo")
        .args([
            "run",
            "-p",
            "atriolum-server",
            "--",
            "--port",
            "18766",
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
            "http://127.0.0.1:18766/api/2/envelope/",
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
