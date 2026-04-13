# Atriolum

> Apache 2.0 licensed, Sentry-compatible error tracking server written in Rust.

Atriolum (Latin: "small atrium" — a place where things gather) accepts events from any existing Sentry SDK (Python, JavaScript, Rust, Go, C++, etc.) by implementing the Sentry ingestion protocol, and stores them as files on disk.

## Quick Start

```bash
# Build
cargo build --release

# Run (default port 8000, data dir ./data)
cargo run --release -- --port 8000 --data-dir ./data

# With environment variables
ATRIOLUM_PORT=9000 ATRIOLUM_DATA_DIR=/var/lib/atriolum cargo run --release
```

## SDK Configuration

Point any Sentry SDK to Atriolum using a DSN:

```
http://{PUBLIC_KEY}@{ATRIOLUM_HOST}:{PORT}/{PROJECT_ID}
```

Example DSN: `http://testkey@localhost:8000/1`

### Python

```python
import sentry_sdk
sentry_sdk.init(dsn="http://testkey@localhost:8000/1")
```

### JavaScript (Node.js)

```javascript
const Sentry = require("@sentry/node");
Sentry.init({ dsn: "http://testkey@localhost:8000/1" });
```

### Rust

```rust
let _guard = sentry::init(("http://testkey@localhost:8000/1", sentry::ClientOptions::default()));
```

### Go

```go
sentry.Init(sentry.ClientOptions{Dsn: "http://testkey@localhost:8000/1"})
```

## CLI Client

```bash
# Single command mode
atriolum-cli events list --server ws://localhost:8000/ws/cli
atriolum-cli events show <event_id>
atriolum-cli projects list

# Interactive REPL mode
atriolum-cli --server ws://localhost:8000/ws/cli
atriolum> events list --level error --limit 10
atriolum> events show fc6d8c0c43fc4630ad850ee518f1b9d0
atriolum> projects list
atriolum> exit
```

## Architecture

```
Sentry SDKs  →  HTTP POST (Envelope/Store)  →  atriolum-server (hyper+tower)
                                                   ↓
                                            atriolum-ingest (parse, auth, decompress)
                                                   ↓
                                            atriolum-store (filesystem storage)
```

## Workspace Crates

| Crate | Purpose |
|-------|---------|
| `atriolum-protocol` | Sentry protocol types: Event, Envelope, Auth, DSN, Session, Log, Span |
| `atriolum-store` | Filesystem storage layer with `Store` trait |
| `atriolum-ingest` | Envelope parsing, auth validation, decompression |
| `atriolum-server` | hyper+tower HTTP server with WebSocket endpoints |
| `atriolum-cli` | Native CLI client (WebSocket + interactive REPL) |

## Storage Layout

```
data/
  projects/
    {project_id}/
      config.json
      events/{year-month}/{event_id}.json
      transactions/{year-month}/{event_id}.json
      sessions/{date}.jsonl
      logs/{date}.jsonl
      spans/{date}.jsonl
      check_ins/{date}.jsonl
      attachments/{event_id}/{filename}
      profiles/{date}-{id}.json
      replays/{replay_id}/replay.json
      client_reports/{date}-{id}.json
      user_reports/{date}-{id}.json
      raw/{item_type}/{date}-{id}.bin
```

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/{project_id}/envelope/` | Primary Sentry envelope endpoint |
| POST | `/api/{project_id}/store/` | Legacy single event endpoint |
| POST | `/api/{project_id}/minidump/` | C++ SDK minidump crash reports (multipart) |
| POST | `/api/{project_id}/chunk-upload/` | Chunk upload for session replay |
| GET | `/api/health` | Health check |
| GET | `/ws/cli` | WebSocket for native CLI client |
| GET | `/ws/term` | WebSocket for xterm.js web terminal |

## License

Apache License 2.0
