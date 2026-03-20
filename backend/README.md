# Pay-per-Play RPS — Backend

Rock-Paper-Scissors pay-per-play API server. A Rust backend demonstrating HTTP 402-based payment flows using [MPP (Machine Payments Protocol)](https://github.com/tempoxyz/mpp-rs).

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

## API Endpoints

| Method | Path | Auth | Description |
|---|---|---|---|
| `POST` | `/api/play` | MPP (`Authorization: Payment`) | Play a game (402 challenge or result) |
| `GET` | `/api/games/{game_id}` | Public | Game detail |
| `GET` | `/api/receipts/{receipt_id}` | Public | Settlement receipt |
| `GET` | `/api/fairness/{game_id}` | Public | Commit-reveal fairness verification |
| `GET` | `/api/leaderboard` | Public | Leaderboard (sorted by wins) |
| `GET` | `/api/inventory/{wallet_address}` | Public | Token balance by wallet |
| `GET` | `/api/health` | Public | Health check |

### Play Flow

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

### Settlement

| Outcome | Action |
|---|---|
| Win | Payment captured + reward token matching the chosen move |
| Lose | Payment captured, no reward |
| Draw (10 rounds exhausted) | Payment captured, no reward |

## Project Structure

```
src/
├── main.rs              # Entry point, server bootstrap
├── lib.rs               # Library crate (exports for tests)
├── app.rs               # AppState, router configuration
├── config.rs            # Environment variables → AppConfig
├── error.rs             # Unified error type (AppError)
├── routes/              # HTTP handlers
│   ├── play.rs          # POST /api/play (core payment + game flow)
│   ├── games.rs         # GET /api/games/{id}
│   ├── receipts.rs      # GET /api/receipts/{id}
│   ├── fairness.rs      # GET /api/fairness/{id}
│   ├── leaderboard.rs   # GET /api/leaderboard
│   ├── inventory.rs     # GET /api/inventory/{wallet}
│   └── health.rs        # GET /api/health
├── domain/              # Business logic (mostly pure functions)
│   ├── game.rs          # RPS outcome resolution, random choice
│   ├── fairness.rs      # SHA-256 commit-reveal
│   ├── settlement.rs    # Settlement planning + transactional execution
│   └── inventory.rs     # Token balance queries
├── db/                  # PostgreSQL CRUD
│   ├── users.rs
│   ├── games.rs
│   ├── payments.rs
│   ├── settlements.rs
│   └── inventories.rs
├── types/               # DTOs and domain types
│   ├── domain.rs        # GameStatus, Choice, Outcome, Row types
│   └── api.rs           # Request/Response DTOs
├── middleware/           # Tower middleware
│   └── request_id.rs    # X-Request-Id injection
├── jobs/                # Background tasks (Tokio tasks)
│   └── mod.rs           # Game expiration, idempotency cleanup, settlement retry
migrations/
├── 001_initial.sql      # DDL (enum types + 6 tables + indexes)
└── 002_add_rounds.sql   # Add rounds JSONB column to games table
tests/
└── unit_tests.rs        # Domain logic unit tests
```

## Testing

```bash
cargo test
```

Key test areas:

- **RPS outcome resolution** — all 9 possible match-ups
- **Commit-reveal** — generation/verification round-trip, tamper detection
- **Settlement planning** — amounts and rewards for win/draw(capture)/lose
- **State machine** — valid/invalid transition verification
- **Auto-rematch** — round count, ordering, user choice preservation, commit verification

## Deployment

Designed for deployment on Railway:

```bash
# Link project with Railway CLI
railway link

# Deploy after configuring environment variables
railway up
```

Nixpacks automatically detects and builds the Rust project.
