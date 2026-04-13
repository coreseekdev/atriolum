# Architecture

## Crate Dependency Graph

```
atriolum-cli ──────────────────────────┐
                                        │
atriolum-server ─┬─ atriolum-ingest ──┬─┼─ atriolum-protocol
                 │                     │
                 ├─ atriolum-store ────┘
                 │
                 ├─ hyper + tower
                 ├─ tokio-tungstenite
                 └─ clap
```

## Request Flow

```
1. TCP connection accepted (tokio::net::TcpListener)
2. HTTP/1.1 connection served (hyper)
3. Route dispatch (path matching in handle_request)
4. CORS preflight → immediate response
5. WebSocket upgrade → separate handler
6. Ingest endpoint:
   a. Collect body (with 20MB limit)
   b. Decompress (gzip/deflate/identity)
   c. Validate auth (X-Sentry-Auth / query / DSN)
   d. Auto-create project if first request
   e. Parse envelope
   f. Process items (IngestProcessor)
   g. Store via FilesystemStore
   h. Return {"id": "..."}
```

## Key Design Decisions

### Why hyper + tower, not axum?

The routing needs for Atriolum's MVP are minimal (3-4 endpoints). A hand-rolled path matcher in a tower Service keeps the dependency graph small. Post-MVP, migrating to axum is straightforward since tower layers are reusable.

### Why filesystem storage?

Zero operational dependencies. No database to install, configure, or back up. Events are just files. This makes Atriolum trivially self-hostable on any machine. The `Store` trait abstracts the backend, so SQLite/RocksDB can be added later.

### Why auto-create projects?

Sentry SDKs are configured with a DSN that includes the project ID and public key. Requiring a separate project setup step would break the "drop-in replacement" goal. Atriolum creates the project directory and config on first authenticated ingest.

### Why Bytes for envelope payloads?

Envelope items can be JSON (events, transactions, sessions) or binary (attachments, replays). Using `Bytes` avoids copying and supports zero-copy patterns. JSON items are deserialized lazily when the item type demands it.

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
| Raw (unknown) | Binary | Atomic write | `raw/{type}/{date}-{id}.bin` |

**Atomic write pattern**: Write to `.tmp` file, then `fs::rename` to final path. This ensures readers never see partial writes.

**Append pattern (JSONL)**: Open file in append mode, write data + `\n`. Safe for concurrent appends on most filesystems.

## WebSocket Protocol (Planned)

### /ws/cli — JSON-over-WebSocket

```json
// Client → Server
{"type": "events_list", "project": "1", "level": "error", "limit": 20}
{"type": "events_show", "event_id": "abc..."}
{"type": "projects_list"}
{"type": "projects_create", "name": "my-app", "public_key": "..."}
{"type": "ping"}

// Server → Client
{"type": "events", "data": [...]}
{"type": "event_detail", "data": {...}}
{"type": "projects", "data": [...]}
{"type": "pong"}
{"type": "error", "message": "..."}
```

### /ws/term — xterm.js Interactive Shell

- Raw text over WebSocket
- Server parses command lines
- ANSI escape codes for colored output
- Commands: `events list`, `events show <id>`, `projects list`, `projects create <name>`, `help`, `exit`
