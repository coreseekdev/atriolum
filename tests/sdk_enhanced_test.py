"""
Enhanced SDK compatibility test — structured logs, sessions, breadcrumbs, threads, tags, user.
"""
import sentry_sdk
import os
import sys
import json
import time
import logging

DSN = "http://testkey@127.0.0.1:8000/1"
DATA_DIR = "/tmp/atriolum-test"

def test_enhanced_event():
    """Test: event with breadcrumbs, tags, user, extra."""
    sentry_sdk.init(
        dsn=DSN,
        release="test-app@1.0.0",
        environment="testing",
        traces_sample_rate=1.0,
    )

    # Set user
    sentry_sdk.set_user({"id": "user-123", "email": "test@atriolum.dev", "username": "testuser"})

    # Add breadcrumbs
    sentry_sdk.add_breadcrumb(
        category="auth",
        message="User authenticated",
        level="info",
    )
    sentry_sdk.add_breadcrumb(
        category="http",
        message="GET /api/data",
        level="info",
        data={"status_code": 200, "url": "/api/data"},
    )

    # Set tags
    sentry_sdk.set_tag("feature", "enhanced_test")
    sentry_sdk.set_tag("version", "2.0")

    # Set extra context
    sentry_sdk.set_extra("request_id", "req-abc-123")
    sentry_sdk.set_extra("debug_flag", True)

    # Capture an exception with all context
    try:
        result = {"key": "value"}
        x = result["nonexistent_key"]
    except KeyError:
        sentry_sdk.capture_exception()

    sentry_sdk.flush(timeout=5)
    print("PASS: enhanced event with breadcrumbs, tags, user sent")


def test_capture_message_levels():
    """Test: messages at different severity levels."""
    sentry_sdk.capture_message("fatal test", level="fatal")
    sentry_sdk.capture_message("warning test", level="warning")
    sentry_sdk.capture_message("info test", level="info")
    sentry_sdk.capture_message("debug test", level="debug")
    sentry_sdk.flush(timeout=5)
    print("PASS: multi-level messages sent")


def test_structured_logging():
    """Test: structured log integration (if available in SDK version)."""
    try:
        sentry_sdk.init(
            dsn=DSN,
            release="test-app@1.0.0",
            environment="testing",
            _experiments={"enable_logs": True},
        )

        # Try sending logs through the logging integration
        logger = logging.getLogger("atriolum.test")
        logger.warning("This is a structured warning log")
        logger.error("This is a structured error log")

        sentry_sdk.flush(timeout=5)
        print("PASS: structured logging sent (or attempted)")
    except Exception as e:
        print(f"SKIP: structured logging not supported in this SDK version ({e})")


def verify_files():
    """Verify all data was stored on disk."""
    events_dir = os.path.join(DATA_DIR, "projects", "1", "events")
    sessions_dir = os.path.join(DATA_DIR, "projects", "1", "sessions")
    logs_dir = os.path.join(DATA_DIR, "projects", "1", "logs")

    results = {}

    # Count events
    event_count = 0
    if os.path.exists(events_dir):
        for root, dirs, files in os.walk(events_dir):
            for f in files:
                if f.endswith(".json"):
                    event_count += 1
                    path = os.path.join(root, f)
                    with open(path) as fh:
                        event = json.load(fh)
                    level = event.get("level", "?")
                    platform = event.get("platform", "?")
                    msg = event.get("message", "")

                    # Check for exception
                    exc = event.get("exception", {})
                    exc_info = ""
                    if isinstance(exc, dict) and exc.get("values"):
                        first = exc["values"][0]
                        exc_info = f" [{first.get('type', '?')}: {first.get('value', '')[:40]}]"

                    # Check for breadcrumbs
                    bc = event.get("breadcrumbs", {})
                    bc_count = len(bc.get("values", [])) if isinstance(bc, dict) else 0

                    # Check for user
                    user = event.get("user", {})
                    user_info = f" user={user.get('username', '?')}" if isinstance(user, dict) and user else ""

                    # Tags
                    tags = event.get("tags", {})
                    tag_info = f" tags={len(tags)}" if isinstance(tags, dict) else ""

                    desc = (msg or exc_info)[:50]
                    print(f"  Event {event_count}: level={level:7} platform={platform:10}"
                          f" bc={bc_count}{user_info}{tag_info} desc={desc}")

    results["events"] = event_count

    # Count sessions
    session_count = 0
    if os.path.exists(sessions_dir):
        for f in os.listdir(sessions_dir):
            if f.endswith(".jsonl"):
                path = os.path.join(sessions_dir, f)
                with open(path) as fh:
                    for line in fh:
                        line = line.strip()
                        if line:
                            session_count += 1
                            session = json.loads(line)
                            print(f"  Session: status={session.get('status', '?')} "
                                  f"init={session.get('init', '?')} "
                                  f"errors={session.get('errors', 0)}")

    results["sessions"] = session_count

    # Count logs
    log_count = 0
    if os.path.exists(logs_dir):
        for f in os.listdir(logs_dir):
            if f.endswith(".jsonl"):
                path = os.path.join(logs_dir, f)
                with open(path) as fh:
                    for line in fh:
                        line = line.strip()
                        if line:
                            log_count += 1
                            try:
                                entry = json.loads(line)
                                if "items" in entry:
                                    for item in entry["items"]:
                                        print(f"  Log: level={item.get('level', '?')} body={item.get('body', '')[:50]}")
                                else:
                                    print(f"  Log: level={entry.get('level', '?')} body={entry.get('body', '')[:50]}")
                            except json.JSONDecodeError:
                                print(f"  Log (raw): {line[:60]}")

    results["logs"] = log_count

    print(f"\n--- Summary ---")
    print(f"  Events:   {results['events']}")
    print(f"  Sessions: {results['sessions']}")
    print(f"  Logs:     {results['logs']}")

    return results["events"] >= 5  # At least 5 events from our tests


if __name__ == "__main__":
    print("=" * 60)
    print("Atriolum Enhanced SDK Compatibility Test")
    print("=" * 60)

    test_enhanced_event()
    test_capture_message_levels()
    test_structured_logging()

    time.sleep(1)

    print("\n--- Verifying stored data ---")
    ok = verify_files()

    sys.exit(0 if ok else 1)
