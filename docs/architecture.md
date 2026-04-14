# Architecture

## Crate Dependency Graph

```
atriolum-cli ──┐
               │ HTTP REST + WebSocket
               ▼
atriolum-server ─┬─ atriolum-ingest ──┬─ atriolum-protocol
                 │                     │
                 ├─ atriolum-store ────┘
                 │
                 ├─ hyper (HTTP/1.1)
                 ├─ tokio-tungstenite (WebSocket)
                 └─ clap (CLI args)
```

## Request Flow

### SDK Ingest

```
1. TCP connection accepted (tokio::net::TcpListener)
2. HTTP/1.1 connection served (hyper)
3. Route dispatch (path matching in handle_request)
4. CORS preflight → immediate response
5. Ingest endpoint:
   a. Collect body (with 20MB limit)
   b. Decompress (gzip/deflate/brotli/identity)
   c. Extract sentry_key from auth header / query / DSN
   d. Auto-create project if first request
   e. Validate auth against project config
   f. Parse envelope (or wrap legacy store events)
   g. Process items (IngestProcessor):
      - Inject envelope event_id into event payload
      - Route each item to the appropriate Store method
   h. Store via FilesystemStore
   i. Broadcast event to live tail subscribers
   j. Return {"id": "..."} with rate-limit headers
```

### Management API

```
1. GET /api/0/projects/ → list all projects
2. POST /api/0/projects/ → create project (name, public_key, project_id)
3. GET /api/0/projects/{id}/events/ → list events with filters
4. GET /api/0/projects/{id}/stats/ → aggregate statistics
5. GET /api/0/projects/{id}/releases/ → list releases
```

### Live Event Tail (WebSocket)

```
1. CLI connects to /ws/cli via WebSocket
2. Sends {"type": "tail_subscribe"}
3. Server subscribes to project broadcast channel
4. New events are streamed as JSON in real-time
```

## Key Design Decisions

### Why hyper, not axum?

The routing needs are moderate (~20 endpoints). A hand-rolled path matcher keeps the dependency graph small. The server uses hyper's `http1::Builder::serve_connection` with `service_fn` closures, spawning one task per connection.

### Why filesystem storage?

Zero operational dependencies. No database to install, configure, or back up. Events are just files. The `Store` trait abstracts the backend, so SQLite/RocksDB can be swapped in later without changing ingest logic.

### Why auto-create projects?

Sentry SDKs are configured with a DSN that includes the project ID and public key. Requiring a separate project setup step would break the "drop-in replacement" goal. Atriolum creates the project directory and config on first authenticated ingest.

### Why broadcast channels for live tail?

`tokio::sync::broadcast` provides a natural fan-out pattern. Each project gets its own channel in a `DashMap`. When `store_event` writes to disk, it also sends the event summary to the channel. WebSocket tail subscribers receive events in real-time without polling.

### Why inject event_id into payloads?

Sentry's envelope protocol puts `event_id` in the envelope header, not in individual item payloads. When the event item itself lacks an `event_id`, we inject the envelope's `event_id` into the event before storage. This ensures the file on disk has a complete, self-contained event.

## Thread Model

```
Main Thread:
  - CLI parsing
  - Server binding
  - Tracing subscriber setup

Tokio Runtime (multi-thread):
  - One task per TCP connection
  - Body collection: async
  - File I/O: tokio::fs (async)
  - Atomic writes: write .tmp → rename
  - Broadcast: per-project tokio::sync::broadcast::Sender
```

## Storage Strategy

| Data Type | Format | Write Pattern | Directory |
|-----------|--------|---------------|-----------|
| Events | JSON | Atomic write | `events/{year-month}/{id}.json` |
| Transactions | JSON | Atomic write | `transactions/{year-month}/{id}.json` |
| Sessions | JSONL | Append | `sessions/{date}.jsonl` |
| Logs | JSONL | Append | `logs/{date}.jsonl` |
| Spans | JSONL | Append | `spans/{date}.jsonl` |
| Check-ins | JSONL | Append | `check_ins/{date}.jsonl` |
| Attachments | Binary | Atomic write | `attachments/{event_id}/{filename}` |
| Profiles | JSON | Atomic write | `profiles/{date}-{id}.json` |
| Replays | Binary | Atomic write | `replays/{replay_id}/replay.json` |
| Client Reports | JSON | Atomic write | `client_reports/{date}-{id}.json` |
| User Reports | JSON | Atomic write | `user_reports/{date}-{id}.json` |
| Raw (unknown) | Binary | Atomic write | `raw/{type}/{date}-{id}.bin` |

**Atomic write pattern**: Write to `.tmp` file, then `fs::rename` to final path. Readers never see partial writes.

**Append pattern (JSONL)**: Open file in append mode, write data + `\n`. Safe for concurrent appends on most filesystems.

## Store Trait

The `Store` trait in `atriolum-store` defines all storage operations:

**Write operations**: `store_event`, `store_transaction`, `store_session`, `store_attachment`, `store_client_report`, `store_logs`, `store_span`, `store_check_in`, `store_profile`, `store_replay`, `store_user_report`, `store_raw`

**Read operations**: `get_project_config`, `list_projects`, `list_events`, `get_event`, `get_transaction`, `list_transactions`, `get_project_stats`, `list_releases`, `list_attachments`, `get_attachment`

**Management**: `ensure_project`, `delete_project`, `subscribe_events`

## Event Filtering

The `EventFilter` struct supports:
- `level` — severity filter (fatal/error/warning/info/debug)
- `platform` — SDK platform filter
- `environment` — environment tag filter
- `release` — release version filter
- `query` — full-text search across message, exception type/value, logger, culprit, transaction
- `limit` / `cursor` — pagination
- `start` / `end` — time range

## CLI Architecture

The CLI (`atriolum-cli`) uses HTTP REST for all operations:

```
atriolum-cli
  ├── Commands (clap subcommands)
  │   ├── events list/show
  │   ├── projects list/create/delete/show
  │   ├── stats
  │   ├── releases
  │   ├── transactions
  │   ├── tail (WebSocket)
  │   └── ping
  ├── HTTP client (reqwest)
  │   ├── GET /api/0/projects/...
  │   └── POST/DELETE /api/0/projects/...
  ├── WebSocket client (tokio-tungstenite)
  │   └── /ws/cli for live tail
  └── Output formatting
      ├── Tables (comfy-table)
      ├── Colored levels (colored)
      └── JSON output (--format json)
```

The interactive REPL mode parses commands and delegates to the same HTTP functions as single-command mode.

## WebSocket Protocol (/ws/cli)

JSON-over-WebSocket for management operations:

```
// Client → Server
{"type": "ping"}
{"type": "events_list", "project": "1", "level": "error", "limit": 20}
{"type": "events_show", "event_id": "abc...", "project": "1"}
{"type": "projects_list"}
{"type": "projects_create", "name": "my-app", "public_key": "..."}
{"type": "stats", "project": "1"}
{"type": "tail_subscribe"}

// Server → Client
{"type": "pong"}
{"type": "events", "data": [...]}
{"type": "event_detail", "data": {...}}
{"type": "projects", "data": [...]}
{"type": "project", "data": {...}}
{"type": "stats", "data": {...}}
{"type": "error", "message": "..."}
```
