# SDK Compatibility

Verified compatibility with official Sentry SDKs. All tests run against a live Atriolum server.

## Python (sentry-sdk)

**Tested version**: 2.57.0
**Test file**: `tests/sdk_test.py`, `tests/sdk_enhanced_test.py`

| Feature | Status | Notes |
|---------|--------|-------|
| Exception capture | ✅ | `capture_exception()` → Event stored with full stacktrace |
| Message capture | ✅ | `capture_message()` at all levels (fatal/error/warning/info/debug) |
| Breadcrumbs | ✅ | `add_breadcrumb()` → stored in event JSON |
| Tags | ✅ | `set_tag()` → stored in event JSON |
| Extra | ✅ | `set_extra()` → stored in event JSON |
| User context | ✅ | `set_user()` → id, email, username preserved |
| Release/Environment | ✅ | Set via `init()` params |
| Contexts (runtime, trace) | ✅ | Auto-added by SDK |
| Structured logging | ✅ | `_experiments={"enable_logs": True}` → logs stored as JSONL |
| Sessions | ✅ | Sent and stored as JSONL |
| Attachments | ✅ | Stored as binary files |
| Transactions | ✅ | Stored with span data |
| gzip compression | ✅ | Decompressed server-side |
| brotli compression | ✅ | Decompressed server-side |

### DSN Configuration

```python
sentry_sdk.init(dsn="http://testkey@localhost:8000/1")
```

Use `http://` (not `https://`) for local Atriolum instances.

### Envelope Items Sent

Python SDK sends these envelope item types:
- `event` — error/crash events
- `transaction` — performance transactions (when `traces_sample_rate` set)
- `session` — session updates for release health
- `log` — structured log entries (experimental)
- `client_report` — counts of dropped events
- `attachment` — file attachments
- `check_in` — cron monitor check-ins
- `profile` — performance profiles

---

## JavaScript (sentry-javascript)

**Not yet tested** (requires Node.js). Expected to work based on protocol compatibility.

### Expected DSN

```javascript
Sentry.init({ dsn: "http://testkey@localhost:8000/1" });
```

### Envelope Items Expected

JavaScript SDK sends:
- `event`, `transaction`, `session`, `sessions`, `log`, `span`
- `profile`, `profile_chunk`
- `replay_event`, `replay_recording`
- `client_report`, `user_report`, `feedback`
- `check_in`, `attachment`, `trace_metric`, `metric`

---

## Rust (sentry-rust)

**Not yet tested**. Expected to work based on protocol compatibility.

### Expected DSN

```rust
let _guard = sentry::init(("http://testkey@localhost:8000/1", sentry::ClientOptions::default()));
```

### Envelope Items Expected

Rust SDK sends:
- `event`, `transaction`, `session`, `sessions`, `attachment`
- `session_aggregates`, `check_in`

---

## C++ (sentry-native)

**Not yet tested**. Minidump endpoint implemented.

### Expected DSN

```c
sentry_options_set_dsn(opts, "http://testkey@localhost:8000/1");
```

### Envelope Items Expected

C++ SDK sends:
- `event`, `transaction`, `session`, `attachment`, `client_report`
- Minidump crash reports via `POST /api/{id}/minidump/` (multipart/form-data) ✅

---

## Server Feature Matrix

| Feature | Status |
|---------|--------|
| Envelope endpoint | ✅ |
| Legacy store endpoint | ✅ |
| Minidump endpoint (multipart) | ✅ |
| Chunk upload endpoint | ✅ |
| gzip decompression | ✅ |
| deflate decompression | ✅ |
| brotli decompression | ✅ |
| Rate limit response headers | ✅ |
| CORS headers | ✅ |
| Auto-create projects | ✅ |
| Event ID injection | ✅ |
| Management REST API | ✅ |
| Live event tail (WebSocket) | ✅ |
| zstd decompression | ❌ Not yet |
| `sent_at` clock drift correction | ❌ Not yet |

---

## Cross-SDK Compatibility Notes

### Content-Encoding

All SDKs support `gzip` and `identity`. Atriolum handles:
- `identity` (no compression) ✅
- `gzip` ✅
- `deflate` (zlib-wrapped) ✅
- `br` (brotli) ✅
- `zstd` — not yet implemented

### Timestamp Formats

SDKs send timestamps in two formats:
1. **RFC 3339 string**: `"2026-04-13T10:30:00Z"` — Python, JS
2. **Unix epoch number**: `1713000000.0` — JS, Rust, C++

Atriolum handles both via custom deserializer.

### Event ID Format

SDKs send event IDs as 32-char hex strings without dashes:
```
"fc6d8c0c43fc4630ad850ee518f1b9d0"
```

UUID library parses these correctly. When returning IDs in responses, Atriolum returns the hyphenated UUID format.

### CORS

Browser-based SDKs (JavaScript) require CORS headers. Atriolum returns permissive CORS on all endpoints:
```
Access-Control-Allow-Origin: *
```

### Unknown Item Types

If an SDK sends an envelope item type that Atriolum doesn't recognize, it logs a warning and stores it as raw data. Other items in the same envelope are still processed.

### Known Item Types

All these types are recognized and routed to appropriate storage:

`event`, `transaction`, `session`, `sessions`, `attachment`, `client_report`, `user_report`, `feedback`, `user_feedback`, `log`, `span`, `check_in`, `profile`, `profile_chunk`, `replay_event`, `replay_recording`, `statsd`, `metric_meta`, `metric`, `trace_metric`, `raw_security`
