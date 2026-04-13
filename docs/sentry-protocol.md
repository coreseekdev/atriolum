# Sentry Protocol Reference

> What we learned from studying the official Sentry SDKs (Python, JavaScript, Rust, C++).
> Sources: `.sdk-references/sentry-{python,javascript,rust,native}/`

## Authentication

Three mechanisms, tried in order:

### 1. X-Sentry-Auth Header (primary)

```
X-Sentry-Auth: Sentry sentry_version=7, sentry_key=abc123, sentry_client=sentry.python/1.45.0
```

Required fields: `sentry_key`, `sentry_version` (must be `7`).
Optional: `sentry_client`, `sentry_secret` (deprecated).

### 2. Query String Fallback

```
?sentry_version=7&sentry_key=abc123
```

Used when custom headers can't be set (e.g., browser `<script>` tags).

### 3. DSN in Envelope Header

```json
{"event_id":"...","dsn":"https://key@sentry.io/42"}
```

The DSN field in the first line of the envelope can self-authenticate.

### DSN Format

```
{PROTOCOL}://{PUBLIC_KEY}:{SECRET_KEY}@{HOST}{PATH}/{PROJECT_ID}
```

Example: `https://public@sentry.example.com/1`

The SDK constructs the endpoint URL as:
`{PROTOCOL}://{HOST}{PATH}/api/{PROJECT_ID}/envelope/`

---

## Envelope Format

Newline-delimited multipart format:

```
{envelope_header_json}\n
{item_header_json}\n
{item_payload}\n
{item_header_json}\n
{item_payload}\n
```

### Envelope Header

```json
{
  "event_id": "fc6d8c0c43fc4630ad850ee518f1b9d0",
  "dsn": "https://key@sentry.io/42",
  "sdk": {"name": "sentry.python", "version": "1.45.0"},
  "sent_at": "2026-04-13T10:30:00Z"
}
```

### Item Header

```json
{"type": "event", "length": 35, "content_type": "application/json"}
```

- `type` (required): item type string
- `length` (recommended): payload byte count; if absent, payload runs to next `\n`
- `content_type` (optional): MIME type

---

## Supported Envelope Item Types

### Core

| Type | Description | Payload |
|------|-------------|---------|
| `event` | Error/exception event | JSON (Event) |
| `transaction` | Performance transaction | JSON (Event with spans) |
| `attachment` | Binary/text file | Raw bytes |

### Release Health

| Type | Description | Payload |
|------|-------------|---------|
| `session` | Single session update | JSON (SessionUpdate) |
| `sessions` | Aggregated session counts | JSON (SessionAggregates) |

### Performance

| Type | Description | Payload |
|------|-------------|---------|
| `span` | Individual span | JSON (Span) |
| `profile` | Performance profile | JSON |
| `profile_chunk` | Profile chunk v2 | JSON |

### Monitoring

| Type | Description | Payload |
|------|-------------|---------|
| `check_in` | Cron monitor check-in | JSON (CheckIn) |
| `client_report` | Dropped event counts | JSON |

### Replay

| Type | Description | Payload |
|------|-------------|---------|
| `replay_event` | Session replay event | JSON |
| `replay_recording` | Session replay recording | Binary (gzipped) |

### Feedback

| Type | Description | Payload |
|------|-------------|---------|
| `user_report` | User feedback form | JSON |

### Logs & Metrics

| Type | Description | Payload |
|------|-------------|---------|
| `log` | Structured log entries | JSON (LogBatch: `{items: [...]}`) |
| `statsd` | Metrics in statsd format | Text |
| `metric_meta` | Per-metric metadata | JSON |

---

## Event Payload

### Core Fields

```json
{
  "event_id": "fc6d8c0c43fc4630ad850ee518f1b9d0",
  "timestamp": "2026-04-13T10:30:00Z",
  "platform": "python",
  "level": "error",
  "logger": "myapp.errors",
  "transaction": "GET /api/users",
  "server_name": "web-01",
  "release": "myapp@1.0.0",
  "dist": "abc123",
  "environment": "production"
}
```

**Timestamp**: Can be RFC 3339 string OR numeric Unix epoch (float or int).
**Level**: `fatal`, `error`, `warning`, `info`, `debug`.
**event_id**: 32-char hex UUID v4, lowercase, no dashes.

### Exception

```json
{
  "exception": {
    "values": [
      {
        "type": "ZeroDivisionError",
        "value": "division by zero",
        "module": "__main__",
        "thread_id": 12345,
        "stacktrace": {
          "frames": [
            {
              "filename": "app.py",
              "abs_path": "/home/user/app.py",
              "function": "main",
              "module": "__main__",
              "lineno": 42,
              "colno": 5,
              "in_app": true,
              "context_line": "result = 1 / 0",
              "pre_context": ["line 40", "line 41"],
              "post_context": ["line 43", "line 44"]
            }
          ]
        }
      }
    ]
  }
}
```

Frames are ordered **bottom-up**: the last frame is the outermost (entry point), the first frame is where the error occurred.

### Breadcrumbs

```json
{
  "breadcrumbs": {
    "values": [
      {
        "type": "http",
        "category": "xhr",
        "message": "GET /api/data",
        "level": "info",
        "timestamp": "2026-04-13T10:29:55Z",
        "data": {
          "url": "/api/data",
          "status_code": 200,
          "method": "GET"
        }
      }
    ]
  }
}
```

Breadcrumb types: `default`, `http`, `navigation`, `user`, `info`, `error`, `system`.

### Threads

```json
{
  "threads": {
    "values": [
      {
        "id": 1,
        "name": "MainThread",
        "crashed": false,
        "current": true,
        "stacktrace": { "frames": [...] }
      }
    ]
  }
}
```

### User

```json
{
  "user": {
    "id": "user-123",
    "email": "user@example.com",
    "ip_address": "192.168.1.1",
    "username": "johndoe",
    "segment": "premium"
  }
}
```

All fields optional. Can contain arbitrary extra fields.

### Request

```json
{
  "request": {
    "url": "https://example.com/api/users",
    "method": "GET",
    "data": "{}",
    "query_string": "page=1",
    "cookies": "session=abc",
    "headers": {
      "Host": "example.com",
      "User-Agent": "Mozilla/5.0"
    },
    "env": {
      "REMOTE_ADDR": "192.168.1.1"
    }
  }
}
```

### Contexts

```json
{
  "contexts": {
    "os": {
      "name": "Linux",
      "version": "6.1.0"
    },
    "runtime": {
      "name": "CPython",
      "version": "3.14.4",
      "build": "..."
    },
    "trace": {
      "trace_id": "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4",
      "span_id": "b0963599207154f1",
      "parent_span_id": null,
      "op": "http.server",
      "status": "ok"
    },
    "app": {
      "app_identifier": "com.example.app",
      "app_version": "1.0.0"
    },
    "device": {
      "arch": "x86_64",
      "model": "PC"
    },
    "browser": {
      "name": "Chrome",
      "version": "120.0"
    },
    "gpu": {
      "name": "NVIDIA RTX 4090",
      "vendor_name": "NVIDIA"
    }
  }
}
```

Context is a map of arbitrary string keys to objects. Known keys: `os`, `runtime`, `trace`, `app`, `device`, `browser`, `gpu`, `cloud_resource`, `culture`, `response`, `otel`.

### Tags & Extra

```json
{
  "tags": {"feature": "billing", "priority": "high"},
  "extra": {"debug_info": {"step": 3, "elapsed_ms": 150}}
}
```

Tags: `Map<String, String>` (values are strings).
Extra: `Map<String, Value>` (values can be any JSON).

---

## Session (Release Health)

### Individual Session Update

Envelope item type: `session`

```json
{
  "sid": "550e8400-e29b-41d4-a716-446655440000",
  "init": true,
  "started": "2026-04-13T00:00:00Z",
  "timestamp": "2026-04-13T12:00:00Z",
  "status": "ok",
  "did": "user123",
  "duration": 3600.0,
  "errors": 0,
  "attrs": {
    "release": "myapp@1.0.0",
    "environment": "production",
    "ip_address": "192.168.1.1",
    "user_agent": "Mozilla/5.0..."
  }
}
```

Status: `ok`, `exited`, `crashed`, `abnormal`.

### Aggregated Sessions

Envelope item type: `sessions`

```json
{
  "attrs": {"release": "myapp@1.0.0", "environment": "production"},
  "aggregates": [
    {"started": "2026-04-13", "exited": 100, "errored": 5, "crashed": 1}
  ]
}
```

---

## Structured Logs

Envelope item type: `log`
Content type: `application/vnd.sentry.items.log+json`

```json
{
  "items": [
    {
      "timestamp": 1704067200.0,
      "level": "info",
      "body": "User logged in successfully",
      "trace_id": "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4",
      "span_id": "b2c3d4e5f6a1b2c3",
      "severity_number": 9,
      "attributes": {
        "sentry.severity_text": "info",
        "sentry.severity_number": 9,
        "user.id": "12345"
      }
    }
  ]
}
```

Log levels: `trace`, `debug`, `info`, `warn`, `error`, `fatal`.

Python SDK sends logs via `sentry_sdk.init(_experiments={"enable_logs": True})` + Python `logging` module.

---

## Span (Individual)

Envelope item type: `span`

```json
{
  "span_id": "a1b2c3d4e5f67890",
  "trace_id": "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4",
  "parent_span_id": "1234567890abcdef",
  "op": "http.server",
  "description": "GET /api/users",
  "status": "ok",
  "start_timestamp": 1704067200.0,
  "timestamp": 1704067201.5,
  "tags": {"http.method": "GET"},
  "data": {"http.status_code": 200},
  "origin": "auto.http"
}
```

Sent by JS and Rust SDKs. Span can be standalone or embedded in a transaction event's `spans` array.

---

## Check-In (Cron Monitoring)

Envelope item type: `check_in`

```json
{
  "check_in_id": "abc123def456",
  "monitor_slug": "my-cron-job",
  "status": "ok",
  "duration": 5.2,
  "release": "1.0.0",
  "environment": "production",
  "monitor_config": {
    "schedule": {"type": "crontab", "value": "*/5 * * * *"},
    "checkin_margin": 2,
    "max_runtime": 10,
    "timezone": "UTC"
  }
}
```

Status: `ok`, `error`, `in_progress`.

---

## Client Report

Envelope item type: `client_report`

```json
{
  "timestamp": "2026-04-13T12:00:00Z",
  "discarded_events": [
    {
      "reason": "queue_overflow",
      "category": "error",
      "quantity": 5
    },
    {
      "reason": "ratelimit_backoff",
      "category": "transaction",
      "quantity": 2
    }
  ]
}
```

Reasons: `queue_overflow`, `ratelimit_backoff`, `rate_limit`, `before_send`.
Categories: `error`, `transaction`, `span`, `session`, `log`, `attachment`.

---

## HTTP Headers

### Request Headers

| Header | Purpose |
|--------|---------|
| `X-Sentry-Auth` | Authentication (see above) |
| `Content-Type` | `application/x-sentry-envelope` for envelope, `application/json` for store |
| `Content-Encoding` | `gzip`, `deflate`, `identity` (br/zstd planned) |
| `User-Agent` | SDK identifier: `{name}/{version}` |

### Response Headers

| Header | Purpose |
|--------|---------|
| `X-Sentry-Error` | Error details on non-200 responses |
| `X-Sentry-Rate-Limits` | Per-category rate limit info (429) |
| `Retry-After` | Seconds to wait before retry (429) |

### CORS Headers (on all responses)

```
Access-Control-Allow-Origin: *
Access-Control-Allow-Methods: POST, OPTIONS
Access-Control-Allow-Headers: X-Sentry-Auth, Content-Type, Content-Encoding
```

---

## Size Limits

| Resource | Limit |
|----------|-------|
| Compressed envelope | 20 MB |
| Decompressed envelope | 100 MB |
| Event/transaction item | 1 MB |
| Individual attachment | 100 MB |
| All attachments combined | 100 MB |
| Profile item | 50 MB |
| Monitor check-in | 100 KB |
| Sessions per envelope | 100 |

---

## Server Responses

### Success

```json
HTTP/1.1 200 OK
Content-Type: application/json

{"id": "fc6d8c0c43fc4630ad850ee518f1b9d0"}
```

### Errors

```json
HTTP/1.1 403 Forbidden
Content-Type: application/json
X-Sentry-Error: invalid authentication

{"detail": "invalid authentication"}
```

```json
HTTP/1.1 413 Content Too Large
Content-Type: application/json

{"detail": "envelope exceeds size limit"}
```

```json
HTTP/1.1 429 Too Many Requests
X-Sentry-Rate-Limits: ...
Retry-After: 60

{"detail": "rate limited"}
```

---

## SDK-Specific Notes

### Python SDK (sentry-python)

- Sends: event, transaction, session, log, attachment, client_report, check_in, profile
- Default transport: HTTP with `urllib3`
- Session tracking: auto-enabled, sends on process exit
- Structured logs: `_experiments={"enable_logs": True}` + Python `logging` module
- Contexts auto-added: `os`, `runtime`
- Breadcrumbs: auto-captured for logging, HTTP requests

### JavaScript SDK (sentry-javascript)

- Sends: event, transaction, session, log, span, attachment, client_report, profile, replay_event, replay_recording, user_report, check_in, trace_metric
- Envelope construction: `createEventEnvelope()`, `createSessionEnvelope()`, `createLogEnvelope()`, `createSpanEnvelope()`
- Session tracking: auto in browser (page load), manual in Node
- Replay: records DOM mutations + user input
- Dynamic sampling context (trace propagation headers)

### Rust SDK (sentry-rust)

- Sends: event, transaction, session_update, session_aggregates, attachment, monitor_check_in
- Protocol types defined in `sentry-types/src/protocol/v7.rs`
- Context enum: Device, Os, Runtime, App, Browser, Trace, Gpu, Otel, Response, Other
- Thread ID type: can be `u64` or `String`

### Native SDK (sentry-native / C++)

- Sends: event, transaction, session, attachment, client_report
- Crash handling: minidump generation, breakpad integration
- Envelope uses same wire protocol as other SDKs
- Native crash reports also sent to `/api/{id}/minidump/` endpoint (multipart)

---

## Licensing Context

| Period | Sentry Server License | Notes |
|--------|----------------------|-------|
| 2008 – Nov 2019 | BSD 3-Clause | Fully open source |
| Nov 2019 – late 2023 | Business Source License 1.1 | Non-compete |
| late 2023 – present | Functional Source License (FSL) | Non-compete |

All Sentry **SDKs** remain Apache 2.0 throughout. Atriolum implements the protocol that the SDKs use, not the server.

**Compatible alternatives**:
- GlitchTip (MIT, Python/Django) — the only other permissively-licensed Sentry-compatible server
- No Rust-based Sentry-compatible server existed before Atriolum
