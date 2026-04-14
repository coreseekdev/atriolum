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

### C++

```c
sentry_options_set_dsn(opts, "http://testkey@localhost:8000/1");
```

## CLI Client

```bash
# Single command mode
atriolum-cli events list --project 1 --level error --limit 10
atriolum-cli events show fc6d8c0c43fc4630ad850ee518f1b9d0 --project 1
atriolum-cli projects list
atriolum-cli projects create --name my-app
atriolum-cli stats --project 1
atriolum-cli releases --project 1
atriolum-cli transactions --project 1
atriolum-cli tail                    # live event stream (WebSocket)

# Interactive REPL mode
atriolum-cli
atriolum> events list -p 1 -l error -n 10
atriolum> events show fc6d8c0c43fc4630ad850ee518f1b9d0
atriolum> projects list
atriolum> stats -p 1
atriolum> exit
```

## Architecture

```
Sentry SDKs  →  HTTP POST (Envelope/Store/Minidump)  →  atriolum-server (hyper)
                                                              ↓
                                                       atriolum-ingest (parse, auth, decompress)
                                                              ↓
                                                       atriolum-store (filesystem storage)
                                                              ↓
CLI Client   →  HTTP REST (management API)  +  WebSocket (live tail)
Browser      →  WebSocket (/ws/term, /ws/cli)
```

## Workspace Crates

| Crate | Purpose |
|-------|---------|
| `atriolum-protocol` | Sentry protocol types: Event, Envelope, Auth, DSN, Session, Log, Span, CheckIn |
| `atriolum-store` | Filesystem storage with `Store` trait, broadcast channels for live tail |
| `atriolum-ingest` | Envelope parsing, auth validation, decompression (gzip/deflate/brotli) |
| `atriolum-server` | hyper HTTP server + WebSocket endpoints |
| `atriolum-cli` | Native CLI client (HTTP REST + interactive REPL) |

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

### SDK Ingest (used by Sentry SDKs)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/{project_id}/envelope/` | Primary Sentry envelope endpoint |
| POST | `/api/{project_id}/store/` | Legacy single event endpoint |
| POST | `/api/{project_id}/minidump/` | C++ SDK minidump crash reports (multipart) |
| POST | `/api/{project_id}/chunk-upload/` | Chunk upload for session replay |

### Management API (used by CLI)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/projects/` | List all projects |
| POST | `/api/projects/` | Create a project |
| GET | `/api/projects/{id}/` | Get project details |
| DELETE | `/api/projects/{id}/` | Delete a project |
| GET | `/api/projects/{id}/events/` | List events (with filters) |
| GET | `/api/projects/{id}/events/{eid}/` | Get single event |
| GET | `/api/projects/{id}/transactions/` | List transactions |
| GET | `/api/projects/{id}/stats/` | Project statistics |
| GET | `/api/projects/{id}/releases/` | List releases |
| GET | `/api/projects/{id}/attachments/{eid}/` | List event attachments |

### Event Query Parameters

| Param | Description |
|-------|-------------|
| `level` | Filter by level (fatal/error/warning/info/debug) |
| `platform` | Filter by platform (python/javascript/rust/go/native) |
| `environment` | Filter by environment |
| `release` | Filter by release version |
| `query` | Full-text search (message, exception, logger) |
| `limit` | Max results (default 20) |
| `cursor` | Pagination cursor |

### Other Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/health` | Health check |
| GET | `/ws/cli` | WebSocket for native CLI client |
| GET | `/ws/term` | WebSocket for xterm.js web terminal |
| OPTIONS | * | CORS preflight |

## Content-Encoding Support

| Encoding | Status |
|----------|--------|
| identity (none) | Supported |
| gzip | Supported |
| deflate (zlib) | Supported |
| br (brotli) | Supported |
| zstd | Not yet implemented |

## License

Apache License 2.0
