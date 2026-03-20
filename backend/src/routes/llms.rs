use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;

pub fn router<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new().route("/llms.txt", get(llms_txt))
}

async fn llms_txt() -> impl IntoResponse {
    (
        [
            ("content-type", "text/plain; charset=utf-8"),
            ("cache-control", "public, max-age=3600"),
        ],
        LLMS_TXT,
    )
}

const LLMS_TXT: &str = r#"# Pay-Per-Play RPS

> A provably-fair Rock-Paper-Scissors game with micropayment-gated play using the MPP (Machine Payments Protocol) 402 flow. Players pay per game via on-chain payments; wins earn token rewards. All server choices are committed before payment, enabling post-game fairness verification.

## Payment Flow

The game uses HTTP 402 Payment Required:

1. POST /api/play without Authorization header → 402 response with WWW-Authenticate challenge and server_commit
2. Client pays the challenged amount and obtains a payment receipt
3. POST /api/play again with Authorization: <receipt> header → game resolves, result returned

## Endpoints

- [Play a game](POST /api/play): Submit rock/paper/scissors choice. Returns 402 challenge (no auth) or game result (with auth).
- [Get game details](GET /api/games/{game_id}): Retrieve game state; server choice revealed after resolution.
- [Verify fairness](GET /api/fairness/{game_id}): Verify commit-reveal integrity for all rounds after game resolves.
- [Get receipt](GET /api/receipts/{receipt_id}): Retrieve settlement details including refund and reward amounts.
- [Leaderboard](GET /api/leaderboard): Top 50 players by wins.
- [Inventory](GET /api/inventory/{wallet_address}): Token balances earned from wins.
- [Health](GET /api/health): Service health and DB status.

## API Reference

### POST /api/play

Request body:
```json
{ "choice": "rock" | "paper" | "scissors", "game_id": "<uuid> (optional, to resume a 402-challenged game)" }
```

**Without Authorization header** — returns HTTP 402:
```json
{
  "error": "payment_required",
  "game_id": "<uuid>",
  "amount": "0.05",
  "currency": "USD",
  "server_commit": "<sha256 hex — commit to server's choice before payment>",
  "expires_at": "<rfc3339>"
}
```
Headers: `WWW-Authenticate: <MPP challenge>`, `Cache-Control: no-store`

**With Authorization header** — returns HTTP 200:
```json
{
  "game_id": "<uuid>",
  "result": "win" | "lose" | "draw",
  "user_choice": "rock" | "paper" | "scissors",
  "server_choice": "rock" | "paper" | "scissors",
  "server_salt": "<hex>",
  "server_commit": "<sha256 hex>",
  "rounds": [
    {
      "round": 1,
      "server_choice": "...",
      "server_salt": "...",
      "server_commit": "...",
      "user_choice": "...",
      "result": "win" | "lose" | "draw"
    }
  ],
  "total_rounds": 1,
  "settlement": {
    "reward_token": "<token_type | null>",
    "reward_amount": 0,
    "captured_amount": "0.05"
  },
  "receipt_id": "<uuid>"
}
```
Headers: `Payment-Receipt: <receipt>`

Draws trigger automatic rematch rounds (up to a max limit) at no extra cost.

### GET /api/games/{game_id}

```json
{
  "id": "<uuid>",
  "status": "payment_required" | "payment_authorized" | "play_locked" | "resolved_win" | "resolved_draw" | "resolved_lose" | "settling" | "settled" | "expired",
  "user_choice": "rock" | "paper" | "scissors",
  "result": "win" | "lose" | "draw" | null,
  "price": "0.05",
  "currency": "USD",
  "server_choice": "<revealed after resolution, null before>",
  "server_salt": "<revealed after resolution, null before>",
  "server_commit": "<sha256 hex>",
  "created_at": "<rfc3339>",
  "resolved_at": "<rfc3339 | null>",
  "settled_at": "<rfc3339 | null>"
}
```

### GET /api/fairness/{game_id}

Only available after game resolution. Verifies SHA-256 commit = sha256(game_id + server_choice + server_salt) for every round.

```json
{
  "game_id": "<uuid>",
  "total_rounds": 1,
  "all_verified": true,
  "rounds": [
    {
      "round": 1,
      "server_choice": "scissors",
      "server_salt": "<hex>",
      "original_commit": "<sha256 hex>",
      "recomputed_commit": "<sha256 hex>",
      "verified": true
    }
  ]
}
```

### GET /api/receipts/{receipt_id}

```json
{
  "receipt_id": "<uuid>",
  "game_id": "<uuid>",
  "outcome": "win" | "lose" | "draw",
  "payment_amount": "0.05",
  "refund_amount": "0.00",
  "captured_amount": "0.05",
  "reward_token": "<token_type | null>",
  "reward_amount": 0,
  "settled_at": "<rfc3339 | null>"
}
```

### GET /api/leaderboard

Returns top 50 wallets ordered by wins desc.

```json
[
  {
    "wallet_address": "0x...",
    "total_games": 10,
    "wins": 7,
    "draws": 1,
    "losses": 2
  }
]
```

### GET /api/inventory/{wallet_address}

```json
{
  "wallet_address": "0x...",
  "tokens": [
    { "token_type": "<token>", "balance": 5 }
  ]
}
```

### GET /api/health

```json
{ "status": "ok", "db": "ok", "version": "0.1.0" }
```

## Fairness Model

Before accepting payment, the server commits to its choice via:

```
commit = SHA-256(game_id || ":" || server_choice || ":" || server_salt)
```

The commit is returned in the 402 response body. After the game resolves, the salt and choice are revealed so anyone can recompute and verify the commit via GET /api/fairness/{game_id}.

## Error Responses

All errors return JSON: `{ "error": "<message>" }`

- 400 Bad Request — validation error
- 402 Payment Required — payment needed
- 404 Not Found — resource not found
- 409 Conflict — idempotency or state conflict
- 410 Gone — game expired
- 500 Internal Server Error — server fault
"#;
