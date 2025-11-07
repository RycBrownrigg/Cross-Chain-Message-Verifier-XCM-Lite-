### Product Specification for Cross-Chain Message Verifier (XCM Lite)

#### Overview

The Cross-Chain Message Verifier (XCM Lite) is a lightweight, Rust-based microservice designed to simulate and verify Cross-Consensus Messages (XCM) within a Polkadot-compatible environment. It serves as a portable relay mechanism, enabling developers to test cross-parachain communication without the necessity of establishing a full blockchain infrastructure. This tool is particularly suitable for prototyping applications based on XCM, including asset transfers, governance calls, and data relays between simulated parachains.

**Target Audience**: Blockchain developers, particularly those engaged with Polkadot/Substrate ecosystems, requiring a rapid local tool for XCM validation and simulation.

**Key Value Proposition**:
- Enables rapid iteration on XCM logic in a controlled, isolated environment.
- Demonstrates Rust's strengths in safe concurrency, cryptographic operations, and efficient data encoding/decoding.
- Portable and deployable as a microservice for integration into CI/CD pipelines or dApps.

**Scope**:
- In-Scope: Message signing, verification, relay simulation, basic execution (mock effects like balance updates).
- Out-of-Scope: Full blockchain consensus, real on-chain deployment, advanced XCM instructions (e.g., teleporting assets across real networks).

#### Functional Requirements
1. **Message Submission**:
   - Accept XCM messages via REST API (JSON payload).
   - Support basic XCM instructions: e.g., `TransferReserveAsset`, `Transact`, `QueryResponse`.
   - Require sender parachain ID, destination parachain ID, and message body.

2. **Signing and Verification**:
   - Use ed25519 for parachain keypair signing (simulating parachain authority).
   - Verify signatures, XCM version (e.g., V3/V4), and message integrity.
   - Reject invalid messages with detailed error responses (e.g., "InvalidSignature", "VersionMismatch").

3. **Relay and Execution**:
   - Simulate relay chain forwarding: Route messages between in-memory "parachains."
   - Mock execution on destination: e.g., Update a simple state (balances, logs) and return outcomes.
   - Support multi-hop messages (up to 2-3 hops for simplicity).

4. **Query and Monitoring**:
   - API endpoints to query message status (pending, relayed, executed, failed).
   - Basic logging of message history for debugging.

5. **Configuration**:
   - Configurable via TOML/ENV: Number of simulated parachains (default: 3), keypairs, XCM version.
   - Optional: Enable/disable strict mode (e.g., enforce fee calculations).

#### Non-Functional Requirements
- **Performance**: Handle 100+ messages/sec on modest hardware (leverage Rust's async Tokio for concurrency).
- **Security**: Use secure random for key generation; no persistent storage of private keys.
- **Reliability**: 100% test coverage for core logic; graceful error handling.
- **Scalability**: In-memory state for demo; extensible to Redis for production-like setups.
- **Deployment**: Dockerized; runs on port 8080 by default.
- **Dependencies**: Minimal external crates; no runtime dependencies beyond Rust stdlib equivalents.
- **Documentation**: OpenAPI spec for API, README with setup, examples, and architecture diagram.

#### User Stories
- As a developer, I can submit a signed XCM message and receive confirmation of successful relay/execution.
- As a tester, I can simulate failures (e.g., invalid sig) and inspect error details.
- As an integrator, I can deploy the service and interact via curl/Postman.

#### Assumptions and Constraints
- Assumes familiarity with Polkadot XCM basics.
- Limited to mock environments; not for production blockchain use.
- Rust version: 1.75+ for stable features.

### System Architecture

#### High-Level Design
The system adheres to a modular, layered architecture inspired by best practices in microservices. It emphasizes separation of concerns for ease of maintenance, asynchronous, event-driven processing to enhance efficiency, and trait-based extensibility to facilitate future parachain behaviors.


## System Architecture


```
┌─────────────────┐     HTTP       ┌────────────────────┐
│   Client        │ ─────────────► │   Axum API Layer   │
│ (curl, CLI,     │ ◄───────────── │  POST /submit      │
│  Postman, dApp) │   JSON Resp.   │  GET  /status/:id  │
└─────────────────┘                └─────────▲──────────┘
                                             │
                                             ▼
                                   ┌────────────────────┐
                                   │ Message Processor  │
                                   │ (Tokio Task)       │
                                   └───────▲──────▲─────┘
                                           │      │
                   ┌───────────────────────┘      └──────────────┐
                   ▼                                             ▼
        ┌─────────────────────┐                       ┌─────────────────────┐
        │ Signature Verifier  │                       │  Relay Simulator    │
        │ (ed25519-dalek)     │                       │ (mpsc Channel)      │
        └─────────▲───────────┘                       └───────▲─────────────┘
                  │                                           │
                  │ Valid?                                    │ Forward
                  ▼                                           ▼
        ┌─────────────────────┐             ┌──────────────────────────────┐
        │   Reject + Error    │             │     Execution Engine         │
        │   (JSON Response)   │             │  (Trait: execute_xcm())      │
        └─────────────────────┘             └──────────────▲───────────────┘
                                                           │
                                                           ▼
                                                ┌────────────────────────┐
                                                │ In-Memory State Store  │
                                                │ (Arc<RwLock<HashMap>>) │
                                                │ - Balances             │
                                                │ - Message Log          │
                                                └────────────▲───────────┘
                                                             │
                                                             ▼
                                                ┌────────────────────────┐
                                                │   Logging & Metrics    │
                                                │ (tracing + Prometheus) │
                                                └────────────────────────┘
```


-------

**Components**:
  1. **REST API Layer** (Axum): Handles incoming HTTP requests. Routes for `/submit`, `/status/{msg_id}`, `/config`. Uses middleware for auth (optional JWT) and rate-limiting.
  2. **Message Processor**: Async coordinator using Tokio channels. Parses incoming JSON into XCM structs via `parity-scale-codec`.
  3. **Signature Verifier**: Cryptographic module with `ed25519-dalek`. Checks sender authenticity against configured parachain keys.
  4. **Relay Simulator**: Models the relay chain. Uses a simple queue (Tokio mpsc) to forward messages, simulating delays or failures.
  5. **Execution Engine**: Trait-based executor per parachain. Mocks effects (e.g., `Balances::transfer`). Extensible for custom instructions.
  6. **State Store**: Thread-safe in-memory HashMap (wrapped in Arc<Mutex>) for parachain states (balances, message logs).
  7. **Logging & Metrics**: `tracing` crate for structured logs; optional Prometheus endpoint for monitoring.
  8. **Error Handler**: Custom enum for errors (e.g., `XcmError::InvalidPayload`), propagated as JSON responses.

#### Data Flow
1. Client sends POST `/submit` with JSON: `{ "sender_para": 1000, "dest_para": 2000, "xcm": { ... }, "signature": "..." }`.
2. API deserializes (Serde) and spawns Tokio task for processing.
3. Verifier checks sig → If valid, enqueue in Relay.
4. Relay forwards to dest_para's Execution Engine.
5. Engine applies message → Updates state → Logs outcome.
6. Response: `{ "status": "Executed", "outcome": { "new_balance": 100 } }` or error.

#### Tech Stack Breakdown
| Layer | Crates/Tools | Rationale |
|-------|--------------|-----------|
| Web Server | Axum + Tower | Lightweight, async, composable middleware. |
| Encoding/Decoding | parity-scale-codec | Standard for Substrate/Polkadot data serialization. |
| Crypto | ed25519-dalek + rand | Secure signing/verification; RNG for keygen. |
| Async Runtime | Tokio | Handles concurrency for simulation without threads exploding. |
| Config | config-rs or env | Simple TOML/ENV parsing. |
| Testing | actix-test or axum-test | Integration tests for API; unit for crypto/logic. |
| Docs | utoipa (for OpenAPI) | Auto-generates Swagger UI. |

#### Potential Extensions
- Integrate with real Polkadot nodes via `subxt` for hybrid mode.
- Add WebSocket for real-time message updates.
- Persistence with SQLx for durable state.
