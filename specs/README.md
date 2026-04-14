# Atriolum Allium Specifications

This directory contains Allium specifications for the Atriolum Sentry-compatible error tracking server.

## Specification Files

### atriolum.allium
The main specification covering:
- Core entities: Project, StoredEvent, SessionUpdate, StoredAttachment, CheckIn
- Authentication and envelope processing rules
- WebSocket event streaming
- Management API operations
- Key invariants and guarantees

### protocol.allium
Protocol type definitions covering:
- Event structure and fields
- Envelope format and item types
- Session and check-in data models
- Span/tracing structures
- DSN and authentication types
- Protocol-level invariants

### storage.allium
Filesystem storage layer covering:
- Project and event file storage
- Attachment storage
- Session JSONL storage
- Broadcast channels for WebSocket streaming
- Atomic write guarantees
- Path computation and partitioning

### ingest.allium
Request processing pipeline covering:
- HTTP request validation
- Body decompression (gzip, brotli)
- Authentication (header, query, envelope DSN)
- Envelope parsing
- Item processing and routing
- Rate limiting

## Key Entities

| Entity | Purpose |
|--------|---------|
| `Project` | A project that receives events, identified by project_id and public_key |
| `StoredEvent` | An error event or transaction stored on disk |
| `SessionUpdate` | Session state changes from SDKs |
| `CheckIn` | Cron monitor heartbeat/status |
| `StoredAttachment` | Files associated with events (minidumps, logs, etc.) |
| `Envelope` | The wire format for event submission |
| `AuthCredentials` | Validation state for authentication |
| `WebSocketConnection` | Live event streaming clients |

## Key Rules

### Authentication
- `ValidateEnvelopeAuth`: Validates credentials from header, query, or envelope DSN
- `AuthenticateViaHeader`: X-Sentry-Auth header validation
- `AuthenticateViaQuery`: Query parameter fallback
- `AuthenticateViaEnvelopeDsn`: Extract DSN from envelope header

### Processing
- `ProcessEnvelopeItems`: Routes each envelope item to appropriate handler
- `ProcessEventItem`: Stores events and transactions
- `ProcessAttachmentItem`: Stores file attachments
- `ProcessSessionItem`: Appends to session JSONL files
- `ProcessCheckInItem`: Stores cron monitor status

### Storage
- `StoreEventFile`: Atomic writes with temp file + rename
- `AppendToSessionFile`: Append-only JSONL writes
- `StoreAttachmentFile`: Per-event attachment directories
- `BroadcastStoredEvent`: Pushes to WebSocket subscribers

### API
- `ListProjects`, `GetProject`, `DeleteProject`: Project management
- `ListEvents`, `GetEvent`: Event querying with filters
- `GetProjectStats`: Aggregated project statistics

## Key Invariants

- `UniqueEventIds`: No two events share the same event_id
- `ProjectHasEvents`: All events belong to a valid project
- `AtomicWrites`: Enabled writes use temp file + rename pattern
- `BroadcastCapacityNotExceeded`: Channel subscribers never exceed capacity
- `AuthVersionSeven`: Only protocol version 7 is accepted

## Surfaces

### EnvelopeIngest
SDK ingestion endpoints (`/api/{project_id}/envelope`, `/store`)

### ManagementApi
Project and event management API (`/api/projects/...`)

### EventStreaming
WebSocket endpoint for live event tailing (`/ws/cli`)

### EventStorage
Internal storage write operations

### EventRetrieval
Internal storage read operations

## Config Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `max_compressed_size` | 20MB | Maximum compressed request body |
| `max_event_size` | 1MB | Maximum individual event size |
| `broadcast_capacity` | 256 | WebSocket channel capacity |
| `auto_create_projects` | true | Create projects on first event |
| `ws_ping_interval` | 1 minute | WebSocket heartbeat interval |
| `ws_stale_timeout` | 5 minutes | Close stale connections |
| `rate_limit_per_minute` | 100 | Per-project request limit |
| `accepted_protocol_version` | 7 | Sentry protocol version |
| `atomic_writes` | true | Use atomic file writes |

## Open Questions

Each specification file ends with `open question` declarations for unresolved design decisions:

- Per-project rate limiting tiers
- Retention policy for events and attachments
- File compaction strategy for old partitions
- Behavior for envelopes with mixed valid/invalid items
- Support for request queuing during rate limit exhaustion

## Usage

These specifications describe the **observable behaviour** of the system, not implementation details. They can be used to:

1. **Generate tests** - Verify the implementation matches the spec
2. **Document contracts** - Clear API boundaries and guarantees
3. **Guide changes** - Ensure modifications don't violate invariants
4. **Onboard contributors** - Understand system behaviour without reading code

## Version

All specifications use `-- allium: 3` language version.
