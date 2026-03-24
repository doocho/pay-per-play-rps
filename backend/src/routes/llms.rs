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

> A provably-fair Rock-Paper-Scissors game with micropayment-gated play using the MPP (Machine Payments Protocol) 402 flow. Players pay per game via on-chain payments; wins earn token rewards. Supports PvE (player vs server) and PvP (player vs player) modes.

## Getting Started with Tempo Wallet CLI

All payments use the Tempo network via the `tempo` CLI. Install and set up your wallet before playing:

```bash
# Install Tempo CLI
npm install -g @tempoxyz/cli

# Create or import a wallet
tempo wallet create
# or
tempo wallet import <private-key>

# Check your balance
tempo wallet balance

# Make a payment (used automatically by the 402 flow)
tempo request <method> <url> [--body '<json>']
```

The `tempo request` command handles the full MPP 402 flow automatically:
1. Sends the initial request → receives 402 + WWW-Authenticate challenge
2. Signs and submits the on-chain payment
3. Retries the request with the Authorization header containing the payment receipt

## Payment Flow

The game uses HTTP 402 Payment Required:

1. POST /api/play without Authorization header → 402 response with WWW-Authenticate challenge and server_commit
2. Client pays the challenged amount using `tempo request` or any MPP-compatible wallet
3. POST /api/play again with Authorization: <receipt> header → game resolves, result returned

## Endpoints

### PvE (Player vs Server)

- [Play a game](POST /api/play): Submit rock/paper/scissors choice. Returns 402 challenge (no auth) or game result (with auth).
- [Get game details](GET /api/games/{game_id}): Retrieve game state; server choice revealed after resolution.
- [Verify fairness](GET /api/fairness/{game_id}): Verify commit-reveal integrity for all rounds after game resolves.
- [Get receipt](GET /api/receipts/{receipt_id}): Retrieve settlement details including refund and reward amounts.

### PvP (Player vs Player)

- [Create room](POST /api/pvp/create): Create a PvP room (402 flow). Returns game_id + room_code.
- [Join room](POST /api/pvp/join/{room_code}): Join a PvP room (402 flow). Returns game_id.
- [Enter queue](POST /api/pvp/queue): Enter matchmaking queue (402 flow). Returns game_id + matched status.
- [Leave queue](DELETE /api/pvp/queue): Leave matchmaking queue. Requires Authorization header.
- [Pay for game](POST /api/pvp/pay/{game_id}): Pay for game (402 flow, separate payment step).
- [Submit commit](POST /api/pvp/commit/{game_id}): Submit choice commit hash. Requires Authorization.
- [Reveal choice](POST /api/pvp/reveal/{game_id}): Reveal choice + salt. Requires Authorization.
- [Poll game](GET /api/pvp/game/{game_id}): Poll game state. Optional Authorization for player perspective.
- [Verify PvP fairness](GET /api/pvp/fairness/{game_id}): Verify both players' commit-reveal integrity.

### Shared

- [Leaderboard](GET /api/leaderboard): Top 50 players by wins (PvE + PvP combined).
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

### PvE Fairness

Before accepting payment, the server commits to its choice via:

```
commit = SHA-256(game_id || ":" || server_choice || ":" || server_salt)
```

The commit is returned in the 402 response body. After the game resolves, the salt and choice are revealed so anyone can recompute and verify the commit via GET /api/fairness/{game_id}.

### PvP Fairness

In PvP, both players generate their own salt and compute their commit hash client-side:

```
commit = SHA-256(game_id || ":" || choice || ":" || salt)
```

Both commits must be submitted before either player can reveal. After both reveals, the server verifies each commit and resolves the game. Verify via GET /api/pvp/fairness/{game_id}.

## PvP (Player vs Player) Mode

PvP mode allows two agents/players to compete against each other. Both players pay the same amount; the winner takes the pot minus a platform fee. Uses the same MPP 402 flow and commit-reveal fairness scheme.

### PvP Flow

1. **Create/Join**: Player 1 creates a room (POST /api/pvp/create with 402 flow) or enters matchmaking queue (POST /api/pvp/queue with 402 flow). Player 2 joins via room code (POST /api/pvp/join/{room_code} with 402 flow) or gets matched via queue.
2. **Commit**: Both players independently compute SHA-256(game_id || ":" || choice || ":" || salt) and submit their commit hash (POST /api/pvp/commit/{game_id}).
3. **Reveal**: Both players reveal their choice and salt (POST /api/pvp/reveal/{game_id}). Server verifies commits and resolves the game.
4. **Poll**: Players can poll game state via GET /api/pvp/game/{game_id} at any time.
5. **Draw**: On draw, the game resets to the commit phase for a rematch round (up to 10 rounds).

### PvP Quick Start with Tempo CLI

```bash
# Player A: Create a room
tempo request POST https://<host>/api/pvp/create
# → Returns: { "game_id": "<uuid>", "room_code": "ABC123" }

# Player B: Join the room
tempo request POST https://<host>/api/pvp/join/ABC123
# → Returns: { "game_id": "<uuid>", "status": "both_paid" }

# Both players: Compute commit and submit
# commit = SHA-256(game_id + ":" + choice + ":" + salt)
tempo request POST https://<host>/api/pvp/commit/<game_id> --body '{ "commit": "<sha256 hex>" }'

# Both players: Reveal choice and salt
tempo request POST https://<host>/api/pvp/reveal/<game_id> --body '{ "choice": "rock", "salt": "<hex>" }'

# Poll game state at any time
curl https://<host>/api/pvp/game/<game_id>
```

### POST /api/pvp/create

Creates a PvP room. Uses 402 flow — first call returns payment challenge, second call (with Authorization) creates the room.

**With Authorization** — returns HTTP 200:
```json
{
  "game_id": "<uuid>",
  "room_code": "ABC123",
  "status": "waiting_for_opponent"
}
```

### POST /api/pvp/join/{room_code}

Joins an existing PvP room by room code. Uses 402 flow.

**With Authorization** — returns HTTP 200:
```json
{
  "game_id": "<uuid>",
  "status": "both_paid"
}
```

### POST /api/pvp/queue

Enters the matchmaking queue. Uses 402 flow. If a match is found, both players are paired immediately.

**With Authorization** — returns HTTP 200:
```json
{
  "game_id": "<uuid>",
  "status": "waiting_for_opponent" | "both_paid",
  "matched": true | false
}
```

### DELETE /api/pvp/queue

Leaves the matchmaking queue. Requires Authorization header.

### POST /api/pvp/pay/{game_id}

Separate payment step for room creators who need to pay after creation. Uses 402 flow.

### POST /api/pvp/commit/{game_id}

Submit a commit hash for your choice. Requires Authorization header.

Request body:
```json
{ "commit": "<sha256 hex of game_id:choice:salt>" }
```

Response:
```json
{
  "game_id": "<uuid>",
  "status": "player1_committed" | "player2_committed" | "both_committed"
}
```

### POST /api/pvp/reveal/{game_id}

Reveal your choice and salt. Server verifies against your commit. If both players have revealed, the game resolves.

Request body:
```json
{ "choice": "rock" | "paper" | "scissors", "salt": "<hex>" }
```

**If waiting for opponent reveal:**
```json
{
  "game_id": "<uuid>",
  "status": "player1_revealed" | "player2_revealed"
}
```

**If both revealed (game resolved):**
```json
{
  "game_id": "<uuid>",
  "result": "win" | "lose" | "draw",
  "your_choice": "rock",
  "opponent_choice": "scissors",
  "settlement": {
    "winner_payout": "0.095",
    "platform_fee": "0.005",
    "reward_token": "<token_type | null>",
    "reward_amount": 1
  }
}
```

On draw, the game automatically resets to the commit phase for a rematch (up to 10 rounds).

### GET /api/pvp/game/{game_id}

Poll game state. Optional Authorization header to see your player perspective.

```json
{
  "game_id": "<uuid>",
  "status": "waiting_for_opponent" | "both_paid" | "player1_committed" | "both_committed" | "resolved_player1_wins" | "settled" | ...,
  "room_code": "ABC123",
  "your_player": 1 | 2 | null,
  "result": "win" | "lose" | "draw" | null,
  "your_choice": "rock" | null,
  "opponent_choice": "scissors" | null,
  "rounds": [...],
  "settlement": { ... } | null,
  "created_at": "<rfc3339>"
}
```

Opponent choice is only revealed after the game resolves.

### GET /api/pvp/fairness/{game_id}

Only available after game resolution. Verifies both players' commit-reveal integrity.

```json
{
  "game_id": "<uuid>",
  "total_rounds": 1,
  "all_verified": true,
  "rounds": [
    {
      "round": 1,
      "player1_choice": "rock",
      "player1_salt": "<hex>",
      "player1_commit": "<sha256 hex>",
      "player1_verified": true,
      "player2_choice": "scissors",
      "player2_salt": "<hex>",
      "player2_commit": "<sha256 hex>",
      "player2_verified": true
    }
  ]
}
```

### PvP Settlement

| Outcome | Winner Payout | Platform Fee |
|---------|---------------|--------------|
| Win     | pot - fee     | 5% (default) |
| Draw    | refund both   | 5% (default) |

Winner also earns 1 token matching their winning choice (same as PvE).

### PvP Timeouts

| Phase | Timeout | Result |
|-------|---------|--------|
| Payment | 60s | Game expired |
| Commit | 30s | Non-committing player forfeits |
| Reveal | 30s | Non-revealing player forfeits |

## Error Responses

All errors return JSON: `{ "error": "<message>" }`

- 400 Bad Request — validation error
- 402 Payment Required — payment needed
- 404 Not Found — resource not found
- 409 Conflict — idempotency or state conflict
- 410 Gone — game expired
- 500 Internal Server Error — server fault
"#;
