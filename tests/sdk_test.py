"""Test Atriolum server with the official Sentry Python SDK."""
import sentry_sdk
import os
import sys
import json
import time

DSN = "http://testkey@127.0.0.1:8000/1"
DATA_DIR = "/tmp/atriolum-test"

def test_basic_exception():
    """Test: capture a ZeroDivisionError."""
    sentry_sdk.init(dsn=DSN)
    try:
        1 / 0
    except ZeroDivisionError:
        sentry_sdk.capture_exception()
    sentry_sdk.flush(timeout=5)
    print("PASS: basic exception sent")

def test_capture_message():
    """Test: capture a text message."""
    sentry_sdk.capture_message("hello from atriolum python sdk test", level="error")
    sentry_sdk.flush(timeout=5)
    print("PASS: capture_message sent")

def test_with_tags():
    """Test: event with tags and extra."""
    sentry_sdk.set_tag("test_tag", "atriolum")
    sentry_sdk.set_extra("extra_info", {"version": "0.1.0"})
    sentry_sdk.capture_message("event with tags and extra", level="warning")
    sentry_sdk.flush(timeout=5)
    print("PASS: tagged event sent")

def verify_files():
    """Verify events were stored on disk."""
    events_dir = os.path.join(DATA_DIR, "projects", "1", "events")
    if not os.path.exists(events_dir):
        print(f"FAIL: events directory not found: {events_dir}")
        return False

    event_count = 0
    for root, dirs, files in os.walk(events_dir):
        for f in sorted(files):
            if f.endswith(".json"):
                path = os.path.join(root, f)
                with open(path) as fh:
                    event = json.load(fh)
                event_count += 1
                msg = str(event.get("message", ""))[:60]
                exc = event.get("exception", {})
                exc_type = ""
                if isinstance(exc, dict):
                    values = exc.get("values", [])
                    if values:
                        exc_type = values[0].get("type", "")
                desc = msg or exc_type or "(no message)"
                print(f"  Event {event_count}: level={event.get('level','?'):7} "
                      f"platform={event.get('platform','?'):10} "
                      f"desc={desc}")

    if event_count == 0:
        print("FAIL: no events found on disk")
        return False

    print(f"\nVERIFIED: {event_count} event(s) stored on disk")
    return True

if __name__ == "__main__":
    print("=" * 60)
    print("Atriolum SDK Compatibility Test - Python (sentry-sdk)")
    print("=" * 60)

    test_basic_exception()
    test_capture_message()
    test_with_tags()

    time.sleep(1)

    print("\n--- Verifying stored events ---")
    ok = verify_files()

    sys.exit(0 if ok else 1)
