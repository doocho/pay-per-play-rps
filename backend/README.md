# Pay-per-Play RPS — Backend

Rock-Paper-Scissors pay-per-play API server. A Rust backend demonstrating HTTP 402-based payment flows using [MPP (Machine Payments Protocol)](https://github.com/tempoxyz/mpp-rs). Supports both PvE (player vs server) and PvP (player vs player) modes.

## Tech Stack

| Layer | Technology |
|---|---|
| Language | Rust (2021 edition) |
| HTTP Framework | Axum 0.8 |
| Async Runtime | Tokio |
| Database | PostgreSQL (SQLx 0.8) |
| Payments | MPP (`mpp` crate — Tempo network) |
| Hashing | SHA-256 (commit-reveal fairness) |

## Prerequisites

- **Rust** ≥ 1.93 (required by Tempo dependencies)
- **PostgreSQL** — Railway managed DB or a local instance

## Quick Start

```bash
# 1. Configure environment variables
cp .env.example .env
# Edit .env with your actual values

# 2. Build & run
cargo run

# Or with Railway CLI
railway run cargo run
```

The server starts at `http://localhost:8080` by default.

## Environment Variables

| Variable | Description | Default |
|---|---|---|
| `DATABASE_URL` | PostgreSQL connection string | (required) |
| `PLAY_PRICE` | Price per game | `0.05` |
| `PLAY_CURRENCY` | Currency code | `USD` |
| `GAME_TTL_SECONDS` | Unpaid game expiration time (seconds) | `300` |
| `MPP_SECRET_KEY` | HMAC secret for challenge verification | (required) |
| `MPP_RECIPIENT` | Tempo recipient wallet address | (required) |
| `MPP_RPC_URL` | Tempo RPC endpoint | `https://rpc.moderato.tempo.xyz` |
| `MPP_REALM` | MPP realm identifier | `pay-per-play-rps` |
| `RUST_LOG` | Log level | `info` |
| `PORT` | HTTP listening port | `8080` |
| `PVP_PRICE` | Price per PvP game (per player) | `0.05` |
| `PVP_CURRENCY` | PvP currency code | `USD` |
| `PVP_PLATFORM_FEE_BPS` | Platform fee in basis points (500 = 5%) | `500` |
| `PVP_PAYMENT_TIMEOUT_SECONDS` | Time to complete payment after joining | `60` |
| `PVP_COMMIT_TIMEOUT_SECONDS` | Time to submit commit after both paid | `30` |
| `PVP_REVEAL_TIMEOUT_SECONDS` | Time to reveal after both committed | `30` |

## API Endpoints

### PvE (Player vs Server)

| Method | Path | Auth | Description |
|---|---|---|---|
| `POST` | `/api/play` | MPP (`Authorization: Payment`) | Play a game (402 challenge or result) |
| `GET` | `/api/games/{game_id}` | Public | Game detail |
| `GET` | `/api/receipts/{receipt_id}` | Public | Settlement receipt |
| `GET` | `/api/fairness/{game_id}` | Public | Commit-reveal fairness verification |

### PvP (Player vs Player)

| Method | Path | Auth | Description |
|---|---|---|---|
| `POST` | `/api/pvp/create` | MPP | Create a room (402 flow) → game_id + room_code |
| `POST` | `/api/pvp/join/{room_code}` | MPP | Join a room (402 flow) → game_id |
| `POST` | `/api/pvp/queue` | MPP | Enter matchmaking queue (402 flow) → game_id |
| `DELETE` | `/api/pvp/queue` | MPP | Leave matchmaking queue |
| `POST` | `/api/pvp/pay/{game_id}` | MPP | Pay for game (separate payment step) |
| `POST` | `/api/pvp/commit/{game_id}` | MPP | Submit choice commit hash |
| `POST` | `/api/pvp/reveal/{game_id}` | MPP | Reveal choice + salt → result |
| `GET` | `/api/pvp/game/{game_id}` | Optional MPP | Poll game state |
| `GET` | `/api/pvp/fairness/{game_id}` | Public | Verify commit-reveal integrity |

### Shared

| Method | Path | Auth | Description |
|---|---|---|---|
| `GET` | `/api/leaderboard` | Public | Leaderboard (PvE + PvP combined) |
| `GET` | `/api/inventory/{wallet_address}` | Public | Token balance by wallet |
| `GET` | `/api/health` | Public | Health check |

### PvE Play Flow

```
1. POST /api/play { "choice": "rock" }
   → 402 Payment Required + WWW-Authenticate header + game_id, server_commit

2. Client completes MPP payment

3. POST /api/play { "choice": "rock", "game_id": "..." }
   + Authorization: Payment ...
   → 200 OK + result, server choice, salt, settlement details, receipt_id
```

### Auto-Rematch on Draw

On a draw, the server automatically re-rolls (up to 10 rounds). Each round generates a new `server_choice`, `server_salt`, and `server_commit` to preserve fairness.

| Scenario | Result |
|---|---|
| Win/loss decided in round 1 | Settled immediately |
| Draw occurs | Auto-rematch (up to 10 rounds) |
| All 10 rounds draw (probability ≈ 0.002%) | Payment captured, no reward |

The response includes all round data, and each round can be independently verified via `/api/fairness/{game_id}`.

### PvP Flow

PvP uses commit-reveal where **both players** generate their own salt and commit hash (unlike PvE where the server commits).

```
1. Player A: POST /api/pvp/create (no auth → 402 → pay → game_id + room_code)
2. Player B: POST /api/pvp/join/{room_code} (no auth → 402 → pay → game_id)
3. Both players: POST /api/pvp/commit/{game_id}
   { "commit": "SHA-256(game_id:choice:salt)" }
4. Both players: POST /api/pvp/reveal/{game_id}
   { "choice": "rock", "salt": "..." }
   → Second reveal triggers resolution + settlement
5. Poll: GET /api/pvp/game/{game_id} at any time
```

Alternatively, use matchmaking: `POST /api/pvp/queue` matches two players automatically.

### PvE Settlement

| Outcome | Action |
|---|---|
| Win | Payment captured + reward token matching the chosen move |
| Lose | Payment captured, no reward |
| Draw (10 rounds exhausted) | Payment captured, no reward |

### PvP Settlement

| Outcome | Action |
|---|---|
| Win | Winner receives pot (price×2) minus platform fee (default 5%) + reward token |
| Lose | Payment captured |
| Draw | Auto-rematch (up to 10 rounds). If all draw, both players refunded minus platform fee |
| Timeout | Non-responding player forfeits, opponent wins |

## Project Structure

```
src/
├── main.rs              # Entry point, server bootstrap
├── lib.rs               # Library crate (exports for tests)
├── app.rs               # AppState, router configuration
├── config.rs            # Environment variables → AppConfig
├── error.rs             # Unified error type (AppError)
├── routes/              # HTTP handlers
│   ├── play.rs          # POST /api/play (PvE payment + game flow)
│   ├── games.rs         # GET /api/games/{id}
│   ├── receipts.rs      # GET /api/receipts/{id}
│   ├── fairness.rs      # GET /api/fairness/{id} (PvE)
│   ├── leaderboard.rs   # GET /api/leaderboard (PvE + PvP combined)
│   ├── inventory.rs     # GET /api/inventory/{wallet}
│   ├── health.rs        # GET /api/health
│   ├── pvp.rs           # PvP endpoints (create/join/queue/commit/reveal/poll)
│   ├── pvp_fairness.rs  # GET /api/pvp/fairness/{id}
│   └── llms.rs          # GET /llms.txt (API docs for AI agents)
├── domain/              # Business logic (mostly pure functions)
│   ├── game.rs          # RPS outcome resolution, random choice
│   ├── fairness.rs      # SHA-256 commit-reveal (shared by PvE + PvP)
│   ├── settlement.rs    # PvE settlement
│   ├── pvp_game.rs      # PvP game logic, state transitions
│   ├── pvp_settlement.rs # PvP settlement (pot split, platform fee)
│   ├── inventory.rs     # Token balance queries
│   └── payer.rs         # MPP payer wallet recovery
├── db/                  # PostgreSQL CRUD
│   ├── users.rs
│   ├── games.rs
│   ├── payments.rs
│   ├── settlements.rs
│   ├── inventories.rs
│   ├── pvp_games.rs     # PvP games CRUD
│   ├── pvp_payments.rs  # PvP payments CRUD
│   ├── pvp_settlements.rs # PvP settlements CRUD
│   └── matchmaking.rs   # Matchmaking queue operations
├── types/               # DTOs and domain types
│   ├── domain.rs        # GameStatus, Choice, Outcome, Row types
│   ├── api.rs           # PvE request/response DTOs
│   ├── pvp.rs           # PvP domain types (PvpGameStatus, PvpOutcome, etc.)
│   └── pvp_api.rs       # PvP request/response DTOs
├── middleware/           # Tower middleware
│   └── request_id.rs    # X-Request-Id injection
├── jobs/                # Background tasks (Tokio tasks)
│   └── mod.rs           # Game expiration, PvP timeouts, settlement retry
migrations/
├── 001_initial.sql      # PvE schema (enum types + 6 tables + indexes)
├── 002_add_rounds.sql   # Add rounds JSONB column to games table
└── 003_pvp.sql          # PvP schema (pvp_games, pvp_payments, pvp_settlements, matchmaking_queue)
tests/
└── unit_tests.rs        # Domain logic unit tests (PvE + PvP)
```

## Testing

```bash
cargo test
```

Key test areas:

- **RPS outcome resolution** — all 9 possible match-ups
- **Commit-reveal** — generation/verification round-trip, tamper detection
- **PvE settlement** — amounts and rewards for win/draw(capture)/lose
- **State machine** — valid/invalid transition verification
- **Auto-rematch** — round count, ordering, user choice preservation, commit verification
- **PvP resolution** — player vs player outcome determination
- **PvP settlement** — pot split, platform fee calculation, draw refunds

## Deployment

Designed for deployment on Railway:

```bash
# Link project with Railway CLI
railway link

# Deploy after configuring environment variables
railway up
```

Nixpacks automatically detects and builds the Rust project.
