## Build Plan

- **Scope Confirmation**  
  - Target endpoints: `/submit`, `/status/:id`, `/config`; no health or log routes.  
  - Supported instructions: `TransferReserveAsset`, `Transact`, `QueryResponse` only.  
  - Message schema requires `sender_para`, `dest_para`, message body (encapsulates signature + version).  
  - Error catalog remains minimal (`InvalidSignature`, `VersionMismatch`, etc.).  
  - Relay supports up to 3 hops; any hop failure reverts processing and returns an error.  
  - Configurable at startup: parachain count (default 3), keypairs, XCM version; strict mode planned later, no hot reload.

- **Project Foundations**  
  - Initialize Rust microservice project (`cargo new xcm-lite`, edition 2021, Rust ≥1.75).  
  - Add dependencies: `axum`, `tokio`, `serde`, `serde_json`, `parity-scale-codec`, `ed25519-dalek`, `rand`, `tracing`, `config`, `utoipa` (OpenAPI).  
  - Establish module layout: `api`, `processor`, `crypto`, `relay`, `execution`, `state`, `config`, `error`.

- **Configuration & State**  
  - Implement TOML/ENV loader for parachain count, keypairs, XCM version defaults.  
  - Model in-memory store (`Arc<RwLock<...>>`) for parachain state and message status log.  
  - Seed configured keypairs; generate defaults if unspecified.

- **Domain & Validation**  
  - Define XCM message structs/enums for supported instructions.  
  - Implement validation pipeline ensuring required fields, version compatibility, signature format.  
  - Maintain `XcmError` enum mapping to HTTP responses with current minimal detail.

- **Cryptography Module**  
  - Build ed25519 key management: load/generate keys, retain only in memory, expose verification API.  
  - Provide helper for signing fixtures in tests.

- **Message Processor & Relay**  
  - Parse submissions, verify signatures/version, enqueue into relay queue (`tokio::mpsc`).  
  - Implement multi-hop routing (max 3) with simulated delays/failure.  
  - On hop failure, mark message failed, revert state changes, respond with error.

- **Execution Engine**  
  - Define trait `execute_xcm` per parachain; implement mock behaviors for supported instructions.  
  - Update balances or logs in state store; capture outcomes for status queries.

- **REST API Layer**  
  - Axum routes: `POST /submit`, `GET /status/:id`, `GET /config`.  
  - Request/response models aligned with schema; ensure consistent error responses.  
  - Add middleware for tracing, request ID, basic rate limit if needed.

- **Observability**  
  - Wire `tracing` subscribers with structured logs; capture spans around submission, relay, execution.  
  - Optional Prometheus exporter if time allows; otherwise document logging expectations.

- **Testing Suite**  
  - Unit tests for crypto verification, state updates, instruction execution.  
  - Integration tests hitting HTTP endpoints for success paths, invalid signature, version mismatch, hop failure rollback.  
  - Consider property tests for message parsing.

- **Documentation & Tooling**  
  - Generate OpenAPI spec via `utoipa` for documented endpoints.  
  - Update README with setup, config samples, curl examples, architecture diagram reflecting 3-hop relay behavior.  
  - Provide Dockerfile and scripts (build/test/run).

- **Deployment & Validation**  
  - Build multi-stage Docker image exposing port 8080 with env-config overrides.  
  - Run local load tests to confirm ~100 msg/sec throughput.  
  - Log residual enhancements (strict mode toggle, additional instructions) for backlog.