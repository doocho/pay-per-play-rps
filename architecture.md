# Pay-per-Play RPS — Architecture

- Version: 0.1
- Status: Draft
- Owner: Doohyun Cho
- Last Updated: 2026-03-19
- Based on: PRD v0.1

---

## 1. System Overview

Pay-per-Play RPS is a paid HTTP endpoint demo built as a rock-paper-scissors game. The system is structured as a Rust backend serving both a JSON API and a web frontend, backed by PostgreSQL for persistence and idempotency.

The architecture prioritizes:

- **Separation of concerns** — payment logic, game logic, fairness, and settlement are isolated modules
- **Exactly-once semantics** — idempotent payment verification and settlement
- **Auditability** — all state transitions are logged and traceable
- **Adaptability** — MPP integration is abstracted behind a trait so the payment layer can evolve independently

---

## 2. High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        Client (Browser)                     │
│                  Next.js / React Frontend                   │
└──────────────────────────┬──────────────────────────────────┘
                           │  HTTPS
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                     Rust Backend (Axum)                      │
│                                                             │
│  ┌──────────┐  ┌────────────┐  ┌────────────┐  ┌────────┐  │
│  │  Routes   │  │ Middleware  │  │  Domain    │  │Payments│  │
│  │          │  │            │  │            │  │        │  │
│  │ /play    │  │ request_id │  │ game       │  │ mpp    │  │
│  │ /games   │  │ idempotency│  │ fairness   │  │adapter │  │
│  │ /receipts│  │ tracing    │  │ settlement │  │verifier│  │
│  │ /fairness│  │ auth       │  │ inventory  │  │pricing │  │
│  │ /leader  │  │            │  │            │  │        │  │
│  └──────────┘  └────────────┘  └────────────┘  └────────┘  │
│                           │                                  │
│                    ┌──────┴──────┐                           │
│                    │  DB Layer   │                           │
│                    │  (SQLx)     │                           │
│                    └──────┬──────┘                           │
└───────────────────────────┼─────────────────────────────────┘
                            │
                            ▼
                     ┌───────────┐
                     │ PostgreSQL │
                     └───────────┘
```

---

## 3. Technology Stack

| Layer | Technology | Rationale |
|---|---|---|
| Language | Rust | Learning goal + performance + safety |
| HTTP Framework | Axum | Async, tower-based, idiomatic Rust |
| Async Runtime | Tokio | Standard async runtime for Rust |
| Database ORM | SQLx | Compile-time checked SQL, async Postgres |
| Primary Store | PostgreSQL | Relational integrity for game/payment/settlement data; also handles idempotency via dedicated table |
| Serialization | Serde | Standard Rust serialization |
| HTTP Client | reqwest | For MPP provider communication |
| Observability | tracing + tracing-subscriber | Structured logging with span context |
| Hashing | sha2 (SHA-256) | Commit-reveal fairness proofs |
| RNG | rand (ChaCha20Rng) | Cryptographically secure server choice generation |
| Frontend | Next.js + React | Modern web UI (separate deployment) |

---

## 4. Module Architecture

```
src/
├── main.rs                  # Entry point, server bootstrap
├── app.rs                   # Router construction, shared state
├── config.rs                # Environment and configuration
├── error.rs                 # Unified error type, into_response impl
│
├── routes/
│   ├── mod.rs
│   ├── play.rs              # POST /api/play — core paid play endpoint
│   ├── games.rs             # GET  /api/games/:game_id
│   ├── receipts.rs          # GET  /api/receipts/:receipt_id
│   ├── fairness.rs          # GET  /api/fairness/:game_id
│   └── leaderboard.rs       # GET  /api/leaderboard
│
├── middleware/
│   ├── mod.rs
│   ├── request_id.rs        # Inject X-Request-Id into every request
│   └── idempotency.rs       # Idempotency key enforcement
│
├── domain/
│   ├── mod.rs
│   ├── game.rs              # Game state machine, resolution logic
│   ├── fairness.rs          # Commit-reveal generation & verification
│   ├── settlement.rs        # Outcome-based settlement orchestration
│   └── inventory.rs         # Token balance management
│
├── payments/
│   ├── mod.rs
│   ├── mpp_adapter.rs       # MPP protocol adapter (trait + impl)
│   ├── verifier.rs          # Payment proof verification
│   └── pricing.rs           # Price lookup per play
│
├── db/
│   ├── mod.rs
│   ├── games.rs             # Game CRUD
│   ├── payments.rs          # Payment record CRUD
│   ├── settlements.rs       # Settlement record CRUD
│   └── inventories.rs       # Inventory balance CRUD
│
└── types/
    ├── mod.rs
    ├── api.rs               # Request/response DTOs
    └── domain.rs            # Internal domain types, enums
```

### Module Dependency Rules

- `routes/` depends on `domain/`, `payments/`, `types/`
- `domain/` depends on `db/`, `types/`
- `payments/` depends on `db/`, `types/`
- `db/` depends on `types/`
- `middleware/` depends on `db/`, `types/`
- No circular dependencies allowed

---

## 5. Core Data Flow

### 5.1 Happy Path — POST /api/play

```
Client                          Server
  │                               │
  │  POST /play {choice, nonce}   │
  │──────────────────────────────▶│
  │                               │── generate server_choice, salt
  │                               │── compute commit = SHA256(game_id ‖ choice ‖ salt)
  │                               │── create Game(CREATED → PAYMENT_REQUIRED)
  │                               │── store commit in DB
  │  402 Payment Required         │
  │  {game_id, amount, commit,    │
  │   payment_requirements}       │
  │◀──────────────────────────────│
  │                               │
  │  MPP payment flow             │
  │  (off-band or inline)         │
  │                               │
  │  POST /play {choice, nonce,   │
  │   game_id}                    │
  │  + Authorization: Payment     │
  │──────────────────────────────▶│
  │                               │── idempotency check
  │                               │── parse_authorization(header)
  │                               │── mpp.verify_credential(&credential)
  │                               │── extract payer wallet from Receipt
  │                               │── upsert user, set game.user_id
  │                               │── transition → PAYMENT_AUTHORIZED
  │                               │── transition → PLAY_LOCKED
  │                               │── resolve game (RPS logic)
  │                               │── transition → RESOLVED_*
  │                               │── settle (reward / capture; auto-rematch on draw)
  │                               │── transition → SETTLED
  │                               │── create receipt
  │  200 OK                       │
  │  {result, server_choice,      │
  │   salt, commit, settlement,   │
  │   receipt_id}                 │
  │◀──────────────────────────────│
```

### 5.2 Idempotent Retry

```
Client                          Server
  │                               │
  │  POST /play (retry, same key) │
  │──────────────────────────────▶│
  │                               │── idempotency key lookup in DB
  │                               │── hit: return stored response
  │  200 OK (cached)              │
  │◀──────────────────────────────│
```

---

## 6. Game State Machine

### 6.1 States

| State | Description |
|---|---|
| `CREATED` | Game record initialized |
| `PAYMENT_REQUIRED` | 402 returned, waiting for payment |
| `PAYMENT_AUTHORIZED` | Payment verified, not yet locked |
| `PLAY_LOCKED` | Game locked for resolution — no further mutation |
| `RESOLVED_WIN` | Player won |
| `RESOLVED_DRAW` | Draw |
| `RESOLVED_LOSE` | Player lost |
| `SETTLING` | Settlement in progress |
| `SETTLED` | Settlement complete, terminal state |
| `EXPIRED` | Unpaid game past TTL |
| `FAILED` | Unrecoverable error during processing |

### 6.2 Transition Diagram

```
CREATED
   │
   ▼
PAYMENT_REQUIRED ──────────────────────▶ EXPIRED
   │                                    (TTL exceeded)
   ▼
PAYMENT_AUTHORIZED
   │
   ▼
PLAY_LOCKED
   │
   ├──▶ RESOLVED_WIN
   ├──▶ RESOLVED_DRAW
   └──▶ RESOLVED_LOSE
          │
          ▼
       SETTLING
          │
          ▼
       SETTLED

Any state may transition to FAILED on unrecoverable error.
```

### 6.3 Implementation Strategy

Game states are represented as a Rust enum. Transitions are enforced via a `transition(from, to) -> Result<GameState>` function that validates legal transitions and logs every change.

```rust
#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type, Serialize, Deserialize)]
#[sqlx(type_name = "game_status", rename_all = "snake_case")]
pub enum GameStatus {
    Created,
    PaymentRequired,
    PaymentAuthorized,
    PlayLocked,
    ResolvedWin,
    ResolvedDraw,
    ResolvedLose,
    Settling,
    Settled,
    Expired,
    Failed,
}

impl GameStatus {
    pub fn can_transition_to(&self, target: &GameStatus) -> bool {
        matches!(
            (self, target),
            (Self::Created, Self::PaymentRequired)
                | (Self::PaymentRequired, Self::PaymentAuthorized)
                | (Self::PaymentRequired, Self::Expired)
                | (Self::PaymentAuthorized, Self::PlayLocked)
                | (Self::PlayLocked, Self::ResolvedWin)
                | (Self::PlayLocked, Self::ResolvedDraw)
                | (Self::PlayLocked, Self::ResolvedLose)
                | (Self::ResolvedWin, Self::Settling)
                | (Self::ResolvedDraw, Self::Settling)
                | (Self::ResolvedLose, Self::Settling)
                | (Self::Settling, Self::Settled)
        )
    }
}
```

---

## 7. Domain Design

### 7.1 Game Resolution

Resolution is pure logic with no side effects:

```rust
pub enum Choice {
    Rock,
    Paper,
    Scissors,
}

pub enum Outcome {
    Win,
    Draw,
    Lose,
}

pub fn resolve(user: &Choice, server: &Choice) -> Outcome {
    match (user, server) {
        _ if user == server => Outcome::Draw,
        (Choice::Rock, Choice::Scissors)
        | (Choice::Paper, Choice::Rock)
        | (Choice::Scissors, Choice::Paper) => Outcome::Win,
        _ => Outcome::Lose,
    }
}
```

#### Auto-Rematch on Draw

When a single round resolves to `Draw`, the server automatically re-rolls with a new `server_choice`, `server_salt`, and `server_commit`. This repeats until the outcome is `Win` or `Lose`, up to a maximum of 10 rounds. Each round's fairness data is preserved for verification. If all 10 rounds draw (probability ≈ 0.002%), the game is settled as a draw: payment is captured with no reward.

### 7.2 Fairness — Commit-Reveal

**Commit phase** (before 402 response):

```
server_choice = random RPS choice (CSPRNG)
server_salt   = random 32 bytes (CSPRNG)
server_commit = SHA256(game_id || server_choice || server_salt)
```

**Reveal phase** (after resolution):

Response includes `server_choice`, `server_salt`, and `server_commit`. Any party can verify:

```
SHA256(game_id || revealed_choice || revealed_salt) == server_commit
```

**Implementation notes:**

- `server_choice` and `server_salt` are stored in the DB at game creation, but never exposed until after `PLAY_LOCKED`
- The commit is returned in the 402 response so the client knows it was generated before their payment
- Verification endpoint recomputes the hash and returns a boolean

### 7.3 Settlement

Settlement is an orchestration step that executes exactly one of three paths:

| Outcome | Action |
|---|---|
| Win | Capture payment + issue reward token to inventory |
| Draw | Auto-rematch: re-roll until win or lose (max 10 rounds). If exhausted, capture payment, no reward |
| Lose | Capture payment, no reward |

Settlement is wrapped in a database transaction. If any step fails, the entire settlement rolls back and the game remains in `RESOLVED_*` state for retry.

```rust
pub struct SettlementPlan {
    pub outcome: Outcome,
    pub captured_amount: Decimal,
    pub refund_amount: Decimal,
    pub reward_token: Option<TokenType>,
    pub reward_amount: u32,
}

impl SettlementPlan {
    pub fn from_outcome(outcome: &Outcome, price: Decimal, user_choice: &Choice) -> Self {
        match outcome {
            Outcome::Win => Self {
                outcome: Outcome::Win,
                captured_amount: price,
                refund_amount: Decimal::ZERO,
                reward_token: Some(user_choice.into()),
                reward_amount: 1,
            },
            Outcome::Draw => Self {
                outcome: Outcome::Draw,
                captured_amount: price,
                refund_amount: Decimal::ZERO,
                reward_token: None,
                reward_amount: 0,
            },
            Outcome::Lose => Self {
                outcome: Outcome::Lose,
                captured_amount: price,
                refund_amount: Decimal::ZERO,
                reward_token: None,
                reward_amount: 0,
            },
        }
    }
}
```

### 7.4 Inventory

A simple balance-tracking model per user per token type:

| Token Type | Earned When |
|---|---|
| ROCK | Win with rock |
| PAPER | Win with paper |
| SCISSORS | Win with scissors |

Inventory updates happen inside the settlement transaction via `INSERT ... ON CONFLICT (user_id, token_type) DO UPDATE SET balance = balance + $1`.

---

## 8. Payment Integration (MPP)

### 8.1 MPP SDK (`mpp` crate)

The payment layer uses the `mpp` Rust SDK directly. No custom adapter trait is needed — the SDK provides the right abstraction.

```rust
use mpp::server::{Mpp, tempo, TempoConfig};
use mpp::{parse_authorization, format_www_authenticate};

// Setup (once at startup, stored in AppState as Arc<Mpp>)
let mpp = Mpp::create(tempo(TempoConfig {
    recipient: "0x...",
}))?;

// 402 path: generate a payment challenge
let challenge = mpp.charge("0.05")?;
let header = format_www_authenticate(&challenge)?;
// → return 402 + WWW-Authenticate header

// 200 path: verify credential from Authorization header
let credential = parse_authorization(auth_header)?;
let receipt = mpp.verify_credential(&credential).await?;
// receipt.reference → transaction reference
// payer wallet address extracted from the verified receipt
```

Key SDK types:

| Type | Role |
|---|---|
| `Mpp` | Server instance binding method, realm, and secret |
| `Challenge` | Payment requirement returned to client |
| `Credential` | Payment proof submitted by client |
| `Receipt` | Verification result with payer info and reference |

### 8.2 Payment Flow within HTTP

```
                    ┌──────────────────┐
                    │  POST /api/play  │
                    └────────┬─────────┘
                             │
              ┌──────────────┴──────────────┐
              │  Has payment proof header?   │
              └──────┬───────────────┬──────┘
                     │ No            │ Yes
                     ▼               ▼
           ┌─────────────┐  ┌───────────────────┐
           │ Create game  │  │ Verify payment    │
           │ Return 402   │  │ via MPP adapter   │
           └─────────────┘  └────────┬──────────┘
                                     │
                              ┌──────┴──────┐
                              │   Valid?    │
                              └──┬──────┬──┘
                                 │ No   │ Yes
                                 ▼      ▼
                           ┌────────┐ ┌──────────────┐
                           │  401   │ │ Resolve game │
                           └────────┘ │ Settle       │
                                      │ Return 200   │
                                      └──────────────┘
```

### 8.3 402 Response Structure

The 402 response follows MPP conventions:

```json
{
  "error": "payment_required",
  "game_id": "game_abc123",
  "amount": "0.05",
  "currency": "USD",
  "payment_protocol": "MPP",
  "payment_requirements": {
    "asset": "USDC",
    "network": "Tempo",
    "max_amount": "0.05",
    "recipient": "0x...",
    "memo": "game_abc123"
  },
  "server_commit": "0x7f3a...",
  "expires_at": "2026-03-19T14:10:00Z"
}
```

---

## 9. Database Schema

### 9.1 ERD

```
┌──────────────┐       ┌──────────────┐       ┌──────────────┐
│    users     │       │    games     │       │  payments    │
├──────────────┤       ├──────────────┤       ├──────────────┤
│ id (PK)      │◀──┐   │ id (PK)      │──────▶│ id (PK)      │
│ wallet_addr  │   └──│ user_id (FK) │       │ game_id (FK) │
│ created_at   │       │ status       │       │ protocol     │
└──────────────┘       │ price        │       │ network      │
                       │ currency     │       │ asset        │
┌──────────────┐       │ user_choice  │       │ amount       │
│ inventories  │       │ server_choice│       │ status       │
├──────────────┤       │ server_salt  │       │ provider_id  │
│ id (PK)      │       │ server_commit│       │ auth_payload │
│ user_id (FK) │       │ result       │       │ receipt_data │
│ token_type   │       │ payment_id   │       │ created_at   │
│ balance      │       │ created_at   │       │ updated_at   │
│ updated_at   │       │ resolved_at  │       └──────────────┘
└──────────────┘       │ settled_at   │
                       └──────┬───────┘       ┌──────────────┐
                              │               │  idempotency │
                       ┌──────┴───────┐       ├──────────────┤
                       │ settlements  │       │ id (PK)      │
                       ├──────────────┤       │ key          │
                       │ id (PK)      │       │ scope        │
                       │ game_id (FK) │       │ response     │
                       │ outcome      │       │ created_at   │
                       │ refund_amt   │       │ expires_at   │
                       │ captured_amt │       └──────────────┘
                       │ reward_token │
                       │ reward_amt   │
                       │ status       │
                       │ created_at   │
                       │ updated_at   │
                       └──────────────┘
```

### 9.2 Key Constraints

- `games.user_id` → `users.id` (FK, **NULLABLE** — set when payment is verified and payer wallet is known)
- `payments.game_id` → `games.id` (FK, UNIQUE)
- `settlements.game_id` → `games.id` (FK, UNIQUE)
- `inventories` has a UNIQUE constraint on `(user_id, token_type)`
- `idempotency.key` has a UNIQUE constraint
- `games.server_salt` and `games.server_choice` are stored encrypted or at minimum never returned until state ≥ `PLAY_LOCKED`

### 9.3 Indexes

| Table | Index | Purpose |
|---|---|---|
| games | `(user_id, created_at DESC)` | User game history |
| games | `(status)` WHERE status = 'payment_required' | Expiration sweep |
| payments | `(provider_payment_id)` UNIQUE | Replay attack prevention |
| inventories | `(user_id, token_type)` UNIQUE | Upsert balance |
| idempotency | `(key)` UNIQUE | Fast lookup |
| idempotency | `(created_at)` | TTL cleanup |

---

## 10. API Design

### 10.1 Endpoints

| Method | Path | Auth | Description |
|---|---|---|---|
| POST | `/api/play` | MPP (Authorization header) | Submit play, receive 402 or result |
| GET | `/api/games/:game_id` | Public (UUID unguessable) | Retrieve game detail |
| GET | `/api/receipts/:receipt_id` | Public (UUID unguessable) | Retrieve settlement receipt |
| GET | `/api/fairness/:game_id` | Public | Verify commit-reveal proof |
| GET | `/api/leaderboard` | Public | Top players and stats |
| GET | `/api/inventory/:wallet_address` | Public | Token balances for a given wallet |
| GET | `/api/health` | Public | Liveness check |

### 10.2 Authentication Model

User identity is derived from the MPP payment credential, not from a custom header.

- **POST /api/play (402 path)**: No identity needed. Game is created without `user_id`.
- **POST /api/play (200 path)**: The `Authorization: Payment ...` header contains a cryptographically signed credential. After `mpp.verify_credential()`, the server extracts the payer's wallet address from the verified `Receipt` and upserts the user.
- **Read endpoints**: All public. Game/receipt IDs are UUIDs (unguessable). Inventory uses wallet address as a path parameter.

This eliminates the need for a separate auth middleware in MVP.

### 10.3 Common Headers

| Header | Direction | Purpose |
|---|---|---|
| `X-Request-Id` | Both | Request tracing |
| `X-Idempotency-Key` | Request | Idempotent retry support |
| `Authorization` | Request | `Payment` scheme credential (MPP standard) |
| `WWW-Authenticate` | Response | `Payment` scheme challenge (on 402) |
| `Payment-Receipt` | Response | MPP receipt reference (on 200 after payment) |

### 10.4 Error Response Format

All errors follow a consistent envelope:

```json
{
  "error": "error_code",
  "message": "Human-readable description",
  "game_id": "game_123",
  "request_id": "req_abc"
}
```

Standard error codes:

| HTTP Status | Error Code | Meaning |
|---|---|---|
| 400 | `invalid_choice` | Choice not in {rock, paper, scissors} |
| 401 | `invalid_payment` | Payment proof failed verification |
| 402 | `payment_required` | MPP payment needed |
| 404 | `not_found` | Game/receipt not found |
| 409 | `game_already_settled` | Duplicate settlement attempt |
| 410 | `game_expired` | Payment window exceeded |
| 500 | `internal_error` | Unexpected failure |

---

## 11. Middleware Stack

Axum middleware layers are applied in this order (outermost first):

```
Request
  │
  ▼
┌───────────────────────┐
│  1. Request ID        │  Inject unique X-Request-Id
├───────────────────────┤
│  2. Tracing           │  Create tracing span with request_id
├───────────────────────┤
│  3. Idempotency       │  Check DB for cached response
├───────────────────────┤
│  4. Route Handler     │  Business logic (MPP auth handled per-route)
└───────────────────────┘
  │
  ▼
Response
```

Note: There is no global auth middleware. User identity is extracted from the MPP credential inside the `/api/play` handler only. All other endpoints are public.

### Idempotency Middleware Detail

- Key: value of `X-Idempotency-Key` header
- Scope: per game_id (derived from request body)
- Storage: `idempotency` table in PostgreSQL with `expires_at` column
- On hit: return stored response directly, skip handler
- On miss: execute handler, store response in `idempotency` table before returning
- Cleanup: background job periodically deletes rows where `expires_at < NOW()`

---

## 12. Shared Application State

```rust
pub struct AppState {
    pub db: PgPool,
    pub mpp: Arc<Mpp>,
    pub config: AppConfig,
}

pub struct AppConfig {
    pub play_price: Decimal,
    pub play_currency: String,
    pub game_ttl_seconds: u64,
    pub idempotency_ttl_seconds: u64,
}
```

`AppState` is constructed once at startup and shared via `Axum::with_state()`.

---

## 13. Security

### 13.1 Identity via MPP Credential (No Custom Auth)

User identity is not established by a custom header. Instead, it is derived from the cryptographically verified MPP payment credential:

- The `Authorization: Payment` header contains a credential signed by the payer's private key
- `mpp.verify_credential()` validates the credential against the Tempo blockchain and returns a `Receipt` containing the payer's wallet address
- The server upserts the user record based on this verified address

This prevents impersonation attacks: an attacker cannot forge a credential without the victim's private key, even if they observe the on-chain payment.

### 13.2 MPP Challenge Binding

Each payment is bound to a specific server-issued challenge:

- The challenge ID is delivered only to the requesting client over TLS
- The credential must reference the exact challenge ID issued by the server
- The server validates challenge authenticity via HMAC (`MPP_SECRET_KEY`)
- Each challenge and payment proof is single-use — replay is rejected by the protocol

This means: even if attacker B sees user A's on-chain payment, B cannot construct a valid credential because B does not possess the challenge ID (delivered to A over HTTPS) nor can B forge A's cryptographic signature.

### 13.3 Replay Prevention

- MPP protocol enforces single-use payment proofs at the protocol level
- Each `provider_payment_id` has a UNIQUE constraint in the DB as a second layer of defense
- Idempotency keys are scoped per game_id

### 13.4 Fairness Data Protection

- `server_choice` and `server_salt` are never returned in any response until the game reaches `PLAY_LOCKED` or later
- The 402 response only includes `server_commit` (the hash)

### 13.5 Settlement Integrity

- Settlement executes in a single database transaction
- The `settlements.game_id` UNIQUE constraint prevents double settlement at the DB level
- Application-level state machine check provides a second layer of defense

### 13.6 Read Endpoint Exposure

All read endpoints are public in MVP. This is acceptable because:

- Game and receipt IDs are UUIDv4 (unguessable, 122 bits of entropy)
- Inventory data (token balances) is not sensitive — similar to on-chain data visibility
- Leaderboard is intentionally public
- No financial actions can be triggered through read endpoints

### 13.7 Input Validation

- `choice` must be one of `rock`, `paper`, `scissors` — validated at deserialization
- All IDs are validated for format before DB lookup
- Payment credentials are validated by the MPP SDK before any state transition

---

## 14. Observability

### 14.1 Structured Logging

All log entries include structured fields via `tracing`:

```rust
#[tracing::instrument(
    skip(state),
    fields(game_id, payment_id, outcome)
)]
async fn handle_play(state: AppState, req: PlayRequest) -> Result<impl IntoResponse> {
    // ...
}
```

### 14.2 Key Spans and Events

| Event | Fields | Level |
|---|---|---|
| Game created | `game_id`, `user_id` | INFO |
| Payment verified | `game_id`, `payment_id`, `amount` | INFO |
| Game resolved | `game_id`, `outcome` | INFO |
| Settlement complete | `game_id`, `settlement_id`, `outcome` | INFO |
| Payment verification failed | `game_id`, `reason` | WARN |
| State transition rejected | `game_id`, `from`, `to` | WARN |
| Settlement failed | `game_id`, `error` | ERROR |

### 14.3 Health Check

`GET /api/health` returns:

```json
{
  "status": "ok",
  "db": "connected",
  "version": "0.1.0"
}
```

---

## 15. Error Handling Strategy

### 15.1 Unified Error Type

A single `AppError` enum covers all error categories:

```rust
pub enum AppError {
    Validation(String),
    PaymentRequired(PaymentRequirementPayload),
    PaymentInvalid(String),
    NotFound(String),
    Conflict(String),
    Gone(String),
    Internal(anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response { /* ... */ }
}
```

### 15.2 Error Propagation

- Domain and DB functions return `Result<T, AppError>`
- Handlers use `?` for ergonomic propagation
- `AppError::into_response` maps each variant to the correct HTTP status and error body

---

## 16. Concurrency and Race Conditions

### 16.1 Double-Play Prevention

When a payment proof arrives, the handler:

1. Acquires a row-level lock on the game record (`SELECT ... FOR UPDATE`)
2. Checks current state is `PAYMENT_REQUIRED`
3. Transitions to `PAYMENT_AUTHORIZED`

This prevents two concurrent requests with the same proof from both resolving.

### 16.2 Settlement Atomicity

The entire resolve + settle flow runs inside a single Postgres transaction:

```sql
BEGIN;
  UPDATE games SET status = 'play_locked' WHERE id = $1 AND status = 'payment_authorized';
  -- resolve
  UPDATE games SET status = 'resolved_win', result = 'win', ...;
  -- settle
  UPDATE games SET status = 'settling';
  INSERT INTO settlements ...;
  INSERT INTO inventories ... ON CONFLICT DO UPDATE ...;
  UPDATE games SET status = 'settled';
COMMIT;
```

If any step fails, the transaction rolls back and the game remains in its pre-transaction state.

---

## 17. Background Jobs

### 17.1 Game Expiration Sweep

A background Tokio task runs periodically (e.g., every 60 seconds) to expire stale games:

```sql
UPDATE games
SET status = 'expired'
WHERE status = 'payment_required'
  AND created_at < NOW() - INTERVAL '5 minutes';
```

### 17.2 Settlement Retry

Games stuck in `RESOLVED_*` state (settlement failed) are retried by a background task:

```sql
SELECT * FROM games
WHERE status IN ('resolved_win', 'resolved_draw', 'resolved_lose')
  AND resolved_at < NOW() - INTERVAL '30 seconds';
```

Each retry re-enters the settlement flow, which is idempotent by design.

### 17.3 Idempotency Key Cleanup

A background Tokio task runs periodically (e.g., every 5 minutes) to remove expired idempotency records:

```sql
DELETE FROM idempotency
WHERE expires_at < NOW();
```

---

## 18. Infrastructure

### 18.1 Railway Deployment

Both MVP and production use Railway as the single platform:

```
┌───────────────────────────────────────┐
│              Railway                  │
│                                       │
│  ┌──────────┐    ┌──────────────┐     │
│  │  Rust    │───▶│  Managed     │     │
│  │  Backend │    │  PostgreSQL  │     │
│  │  (Nixpacks)   │              │     │
│  └──────────┘    └──────────────┘     │
│                                       │
│  Private networking between services  │
│  TLS termination at edge              │
│  Auto-deploy from GitHub              │
└───────────────────────────────────────┘
```

Frontend (Next.js) will be deployed separately (Vercel or Railway) in a later phase.

### 18.2 Local Development

No Docker or docker-compose needed. Run the backend directly with `cargo run` and connect to Railway's managed Postgres:

```bash
# Option A: Direct
DATABASE_URL=postgres://... cargo run

# Option B: Railway CLI (injects env vars automatically)
railway link
railway run cargo run
```

### 18.3 Configuration

All configuration is environment-variable driven:

| Variable | Description | Default |
|---|---|---|
| `DATABASE_URL` | Postgres connection string | — |
| `PLAY_PRICE` | Price per game | `0.05` |
| `PLAY_CURRENCY` | Currency code | `USD` |
| `GAME_TTL_SECONDS` | Unpaid game expiration | `300` |
| `MPP_SECRET_KEY` | HMAC secret for stateless challenge verification | — |
| `MPP_RECIPIENT` | Tempo wallet address to receive payments | — |
| `MPP_RPC_URL` | Tempo RPC endpoint | `https://rpc.tempo.xyz` |
| `RUST_LOG` | Tracing filter directive | `info` |
| `PORT` | HTTP listen port | `8080` |

---

## 19. Testing Strategy

### 19.1 Unit Tests

| Module | What to test |
|---|---|
| `domain::game` | All 9 RPS outcome combinations |
| `domain::fairness` | Commit generation and verification round-trip |
| `domain::settlement` | Plan generation for win/draw/lose |
| `GameStatus` | Legal and illegal state transitions |
| `payments::pricing` | Price lookup correctness |

### 19.2 Integration Tests

| Scenario | Setup |
|---|---|
| Full happy path (win) | Mock MPP adapter, real DB |
| Full happy path (draw/lose) | Mock MPP adapter, real DB |
| 402 flow | No payment proof |
| Idempotent retry | Same idempotency key twice |
| Expired game | Game created, wait past TTL |
| Replay attack | Reuse payment proof for second game |
| Settlement failure + retry | Force settlement error, verify retry succeeds |

### 19.3 Test Infrastructure

- Use `sqlx::test` with a test database for DB integration tests (Railway test DB or local Postgres)
- **Unit tests**: Test domain logic only (game resolution, fairness, settlement) — no MPP dependency needed
- **Integration tests**: Use Tempo testnet (moderato) with test tokens for real payment flow verification
- Idempotency tests run against the same test database

---

## 20. Development Milestones (Architecture View)

### M1 — Skeleton + Core Flow

- [ ] Project scaffolding (Cargo, dependencies, directory structure)
- [ ] AppState, config, error type
- [ ] `POST /api/play` returning 402 (no payment verification yet)
- [ ] Game state machine with DB persistence
- [ ] Commit-reveal generation

### M2 — Payment + Resolution

- [ ] MPP SDK integration (`mpp` crate with `server` + `tempo` features)
- [ ] Payment verification flow
- [ ] Game resolution logic
- [ ] Settlement orchestration
- [ ] `POST /api/play` full happy path

### M3 — Supporting Endpoints

- [ ] `GET /api/games/:game_id`
- [ ] `GET /api/receipts/:receipt_id`
- [ ] `GET /api/fairness/:game_id`
- [ ] `GET /api/leaderboard`
- [ ] `GET /api/inventory/:wallet_address`

### M4 — Middleware + Hardening

- [ ] Request ID middleware
- [ ] Idempotency middleware (Postgres)
- [ ] Game expiration background job
- [ ] Settlement retry background job
- [ ] Structured tracing

### M5 — Frontend + Polish

- [ ] Next.js project setup
- [ ] Play screen with 402 flow
- [ ] Result display with fairness proof
- [ ] Inventory and leaderboard pages
- [ ] Railway deployment configuration

---

## 21. Key Design Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Single endpoint for play | `POST /api/play` handles both 402 and resolution | Matches HTTP-native payment negotiation model; client retries same endpoint after paying |
| State machine in DB | Postgres enum column with application-level transition validation | Simple, auditable, crash-recoverable |
| MPP SDK direct | Use `mpp` crate's `Mpp` struct directly via `Arc<Mpp>` in AppState | SDK already provides the right abstraction; identity derived from verified credential |
| Idempotency in Postgres | `idempotency` table with `expires_at` + periodic cleanup | Single data store; no extra infra; sufficient for MVP throughput |
| Settlement in single TX | All settlement writes in one Postgres transaction | Atomicity guarantees exactly-once semantics |
| SHA-256 for commit | `SHA256(game_id \|\| choice \|\| salt)` | Widely understood, easy to verify client-side |
| Reward as DB inventory | Not on-chain for MVP | Simplicity; on-chain migration planned for v2 |
| Frontend separate | Next.js as independent deployment | Decoupled development; standard React tooling |

---

## 22. Open Questions

| # | Question | Impact |
|---|---|---|
| 1 | ~~What is the exact MPP Rust SDK interface?~~ **Resolved**: `mpp` crate v0.5.0 from `tempoxyz/mpp-rs` | — |
| 2 | ~~Should the 402 response use standard MPP headers or JSON body?~~ **Resolved**: Standard `WWW-Authenticate: Payment` header per MPP spec | — |
| 3 | ~~How is user identity established?~~ **Resolved**: Derived from MPP credential's verified Receipt (payer wallet address) | — |
| 4 | Should receipts be signed by the server for external verifiability? | Receipt trustworthiness |
| 5 | ~~What is the target deployment platform for MVP?~~ **Resolved**: Railway (backend + managed Postgres) | — |
| 6 | Does `mpp.verify_credential()` return the payer's wallet address directly, or does it need to be extracted from the on-chain transaction? | User identification implementation detail |
