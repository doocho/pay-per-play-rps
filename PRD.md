좋아. 아래에 바로 **`PRD.md` 전체 내용**으로 출력할게.

````md
# Pay-per-Play RPS PRD

- Version: 0.1
- Status: Draft
- Owner: Doohyun Cho
- Last Updated: 2026-03-19

---

## 1. Overview

**Pay-per-Play RPS** is a provably-fair rock-paper-scissors service where each play requires payment through **MPP (Machine Payments Protocol)**. After payment, the game resolves immediately and settles programmatically based on the result.

This product is not just a simple game. Its main purpose is to demonstrate:

- HTTP-native payment flow
- `402 Payment Required`-based service monetization
- programmable settlement after service execution
- a small but clear machine-commerce use case
- a Rust backend implementation using MPP primitives

In short, this is a **gameified paid endpoint demo**.

---

## 2. Product Goal

### Primary Goal

Build a small but compelling product that showcases how MPP can be used to monetize an HTTP service request and attach outcome-based settlement logic.

### Secondary Goal

Use the project as a hands-on learning vehicle for:

- Rust backend development
- HTTP middleware design
- payment-aware state machines
- fairness and settlement design
- production-like API/service architecture

---

## 3. Problem Statement

Most API monetization flows still feel manual, account-based, and not machine-native. MPP introduces a model where services can respond with payment requirements and clients can programmatically pay to continue.

However, many demos of machine payments are still too abstract:
- “paid API” examples feel dry
- value exchange is not emotionally intuitive
- post-payment settlement logic is often too simple

This project solves that by turning a paid request into a small interactive service:

- request access to play
- pay through MPP
- receive an immediate outcome
- settle based on win / draw / loss

This makes MPP easier to understand and more fun to demonstrate.

---

## 4. Vision

Create a minimal but polished reference application for **machine-native paid interactions**, where a client pays for a service call and the service returns a result with deterministic settlement semantics.

The long-term vision is that this project can evolve from:
- a game demo
into
- a reusable reference architecture for paid endpoints in Rust

---

## 5. Product Scope

### In Scope for MVP

- Single-player rock-paper-scissors
- User pays for each play through MPP
- Server acts as opponent
- Server fairness via commit-reveal
- Outcome-based settlement:
  - win -> choice-based reward token
  - draw -> auto-rematch until decisive result (max 10 rounds)
  - lose -> payment captured by service
- Result receipt
- Fairness verification page/API
- Inventory of earned tokens
- Leaderboard/basic stats

### Out of Scope for MVP

- PvP matchmaking
- cash redeemable rewards
- token marketplace
- complex tournament systems
- mobile app
- multi-chain reward settlement
- advanced AI strategy agents
- social graph / sharing layer

---

## 6. Target Users

### Primary User

A developer, researcher, or crypto-native builder who:
- wants to understand MPP concretely
- is interested in programmable payments
- appreciates a technically interesting demo
- may inspect API responses and receipts directly

### Secondary User

A general crypto/product audience member who:
- wants to try a fun paid endpoint demo
- can understand payment + game + reward flow through UI

---

## 7. User Stories

### Core User Stories

1. As a user, I want to choose rock, paper, or scissors and pay once to play.
2. As a user, I want the payment flow to feel tightly integrated into the service request.
3. As a user, I want the result immediately after successful payment.
4. As a user, I want proof that the server did not cheat.
5. As a user, I want to see whether I won, drew, or lost, and what happened to my payment.
6. As a user, I want to collect tokens corresponding to the choices I used successfully.
7. As a user, I want to inspect a receipt of the play and settlement.

### Developer-Facing Stories

1. As a developer, I want a clean Rust backend architecture that isolates payment logic from game logic.
2. As a developer, I want payment verification and settlement to be idempotent.
3. As a developer, I want clear state transitions for each game.
4. As a developer, I want to reuse the payment middleware/adaptor for future paid endpoints.

---

## 8. Core Concept

A user submits a play request to a paid endpoint.

If payment has not been provided, the server responds with:
- `402 Payment Required`
- MPP payment requirements
- a game identifier
- server commit hash for fairness

Once the client completes payment and retries the request, the server:
- verifies payment
- locks the game request
- resolves the game
- settles the outcome
- returns result + receipt reference

This makes the game itself a **paid HTTP resource**.

---

## 9. Gameplay Rules

### User Input

The player chooses one of:
- `rock`
- `paper`
- `scissors`

### Payment Requirement

Each play requires payment via MPP.

### Result Rules

- **Win**
  - player receives a token corresponding to the chosen move
  - payment is retained by the service
- **Draw**
  - triggers automatic rematch — server re-rolls until win or lose (max 10 rounds)
  - if all 10 rounds draw (extremely rare), payment is captured with no reward
- **Lose**
  - payment is retained by the service
  - no reward

### Example

If the user chooses `rock`:
- wins -> receives `ROCK` token
- draws -> automatic rematch (up to 10 rounds until decisive result)
- loses -> payment goes to treasury

---

## 10. Reward Model

### MVP Reward Type

The recommended reward model is a **choice-based collectible inventory**.

Reward types:
- `ROCK`
- `PAPER`
- `SCISSORS`

### Representation

For MVP:
- rewards may be tracked in the database only

For v2:
- migrate to ERC-1155 or a similar multi-asset token model

### Why this reward model

- preserves the original product idea
- simple to understand
- supports collection mechanics
- avoids direct cash gambling framing
- creates progression and replay motivation

---

## 11. Fairness Model

Fairness is one of the most important parts of the product.

### Problem

If the server sees the player's move first and then chooses its own move, it can cheat.

### Solution: Commit-Reveal

Before the result is revealed, the server creates:
- `server_choice`
- `server_salt`
- `server_commit = hash(game_id || server_choice || server_salt)`

The commit is stored before resolution.

After payment is verified and the play is resolved, the server reveals:
- `server_choice`
- `server_salt`
- `server_commit`

The client or any observer can recompute the hash to verify that the server committed in advance.

### Fairness UX Requirement

The product should expose:
- commit value
- revealed choice
- revealed salt
- verification status

This should be available through both:
- API
- UI page or expandable panel

---

## 12. Functional Requirements

### 12.1 Game Creation / Payment Requirement

The system must:
- accept a play request with a chosen move
- generate a game record if needed
- generate and store server commit data
- return `402 Payment Required` when payment is missing
- include payment requirements in the response

### 12.2 Payment Verification

The system must:
- verify MPP payment proof / authorization
- associate payment with a specific game
- reject invalid or replayed payment submissions
- enforce expiration window on unpaid games

### 12.3 Game Resolution

The system must:
- lock the game once valid payment is verified
- resolve outcome exactly once
- reveal fairness data
- prevent duplicate settlement

### 12.4 Settlement

The system must:
- handle win / lose correctly
- auto-rematch on draw (re-roll until decisive, max 10 rounds)
- issue reward on win
- capture payment on win and lose (and rare exhausted-draw)
- persist settlement records

### 12.5 Receipts

The system must:
- create a result receipt per resolved game
- expose receipt through API
- show key payment + outcome details

### 12.6 Inventory

The system must:
- track user reward balances
- show balance by token type
- update inventory on each win

### 12.7 Leaderboard / Stats

The system should:
- show total plays
- show wins/draws/losses
- show earned token counts
- optionally show win rate and streaks

---

## 13. Non-Functional Requirements

### Performance

- response after payment verification should feel near-instant for MVP
- result resolution target: under 1 second after valid payment confirmation in normal conditions

### Reliability

- settlement must be idempotent
- duplicate retries must not create multiple outcomes or rewards
- data consistency is more important than raw throughput

### Auditability

- all game state transitions should be logged
- payment verification and settlement events should be inspectable
- fairness proof data should be preserved

### Security

- replay attacks must be prevented
- payment proof reuse must be blocked
- game settlement must happen exactly once
- sensitive secrets must not be exposed in client responses

### Observability

- request ID / game ID tracing should exist
- logs should include payment verification and settlement transitions
- failures should be attributable to a clear lifecycle step

---

## 14. Product Flow

### Happy Path

1. user selects move
2. client calls `POST /play`
3. server responds `402 Payment Required`
4. client completes MPP payment
5. client retries `POST /play` with payment auth/proof
6. server verifies payment
7. server resolves game
8. server settles result
9. client receives response with:
   - result
   - revealed server move
   - fairness proof data
   - settlement summary
   - receipt ID

### Draw / Auto-Rematch Path

1. user pays
2. first round resolves to draw
3. server automatically re-rolls (new server choice + salt + commit each round)
4. repeats until win or lose (max 10 rounds)
5. settlement captures payment, issues reward if final result is win
6. receipt shows all rounds and the decisive result

### Retry Path

1. client retries the same request due to network error
2. server checks idempotency key / game state
3. server returns the existing result instead of re-running settlement

---

## 15. API Requirements

### POST /api/play

Purpose:
- single entry point for paid play lifecycle

Behavior:
- if payment missing or invalid -> return `402 Payment Required`
- if payment valid -> resolve and settle game

#### Request Example

```json
{
  "choice": "rock",
  "client_nonce": "abc123"
}
````

#### 402 Response Example

```json
{
  "error": "payment_required",
  "game_id": "game_123",
  "amount": "0.05",
  "currency": "USD",
  "payment_protocol": "MPP",
  "payment_requirements": {
    "asset": "USDC",
    "network": "Tempo",
    "max_amount": "0.05"
  },
  "server_commit": "0xabc123",
  "expires_at": "2026-03-19T14:10:00Z"
}
```

#### Success Response Example

```json
{
  "game_id": "game_123",
  "result": "win",
  "user_choice": "rock",
  "server_choice": "scissors",
  "server_salt": "0x1234",
  "server_commit": "0xabc123",
  "rounds": [
    { "round": 1, "server_choice": "rock", "result": "draw" },
    { "round": 2, "server_choice": "scissors", "result": "win" }
  ],
  "settlement": {
    "reward_token": "ROCK",
    "reward_amount": 1,
    "captured_amount": "0.05"
  },
  "receipt_id": "rcpt_123"
}
```

---

### GET /api/games/:game_id

Purpose:

* retrieve game detail

Must include:

* status
* result
* price
* fairness data if resolved
* timestamps

---

### GET /api/receipts/:receipt_id

Purpose:

* retrieve payment + settlement receipt

Must include:

* payment summary
* result
* capture data
* reward summary

---

### GET /api/fairness/:game_id

Purpose:

* verify commit-reveal proof

Must include:

* server choice
* salt
* original commit
* recomputed commit
* verification boolean

---

### GET /api/leaderboard

Purpose:

* retrieve top players / summary stats

Should include:

* wallet/user ID
* total games
* wins
* draws
* losses
* token balances

---

## 16. State Machine

The game lifecycle should use explicit states.

### Proposed States

* `CREATED`
* `PAYMENT_REQUIRED`
* `PAYMENT_AUTHORIZED`
* `PLAY_LOCKED`
* `RESOLVED_WIN`
* `RESOLVED_DRAW`
* `RESOLVED_LOSE`
* `SETTLING`
* `SETTLED`
* `FAILED`

### Transition Rules

* `CREATED -> PAYMENT_REQUIRED`
* `PAYMENT_REQUIRED -> PAYMENT_AUTHORIZED`
* `PAYMENT_AUTHORIZED -> PLAY_LOCKED`
* `PLAY_LOCKED -> RESOLVED_WIN | RESOLVED_DRAW | RESOLVED_LOSE`
* `RESOLVED_* -> SETTLING`
* `SETTLING -> SETTLED`

### Constraints

* once `PLAY_LOCKED`, user choice and payment association cannot change
* settlement may execute only once
* retries after `SETTLED` should return stored result

---

## 17. Rust Backend Requirements

### Goal

Use this project as a Rust backend learning and architecture exercise.

### Recommended Stack

* Rust
* Axum
* Tokio
* SQLx
* Postgres
* Redis
* Serde
* tracing
* reqwest
* MPP Rust SDK or adaptor layer

### Architectural Principle

Separate concerns clearly:

* HTTP / routing
* payment protocol adaptor
* domain/game logic
* fairness logic
* settlement logic
* persistence layer

### Recommended Module Layout

```text
src/
  main.rs
  app.rs
  routes/
    play.rs
    games.rs
    receipts.rs
    fairness.rs
  middleware/
    idempotency.rs
    request_id.rs
  domain/
    game.rs
    fairness.rs
    settlement.rs
    inventory.rs
  payments/
    mpp_adapter.rs
    verifier.rs
    pricing.rs
  db/
    games.rs
    payments.rs
    settlements.rs
    inventories.rs
  types/
    api.rs
    errors.rs
```

### Important Rust-Specific Requirements

* state transitions should be explicit and typed where reasonable
* avoid mixing payment verification logic into handlers directly
* payment provider / MPP integration should be hidden behind an adaptor trait if possible
* tracing spans should include `game_id`, `payment_id`, `receipt_id`

---

## 18. Data Model Requirements

### Users

Fields:

* id
* wallet_address or account identifier
* created_at

### Games

Fields:

* id
* user_id
* price
* currency
* status
* user_choice
* server_choice
* server_salt
* server_commit
* result
* payment_receipt_id
* created_at
* resolved_at
* settled_at

### Payments

Fields:

* id
* game_id
* protocol
* network
* asset
* amount
* status
* provider_payment_id
* authorization_payload
* receipt_payload
* created_at
* updated_at

### Settlements

Fields:

* id
* game_id
* outcome
* refund_amount
* captured_amount
* reward_token
* reward_amount
* status
* created_at
* updated_at

### Inventories

Fields:

* id
* user_id
* token_type
* balance
* updated_at

### Idempotency Records

Fields:

* id
* key
* scope
* response_snapshot
* created_at

---

## 19. UX Requirements

### Home Page

Must show:

* product name
* short explanation
* play price
* play CTA

### Game Screen

Must show:

* three move buttons
* payment-required flow
* result state
* fairness proof access

### Result Screen / Panel

Must show:

* user move
* server move
* outcome
* settlement details
* receipt link/reference

### Inventory Page

Must show:

* ROCK count
* PAPER count
* SCISSORS count
* collection progress

### Leaderboard Page

Should show:

* total plays
* wins/losses/draws
* top collectors
* rank

### Fairness Page

Must show:

* commit
* revealed server move
* revealed salt
* recomputed hash
* verified or not

---

## 20. Pricing Strategy

### MVP Recommendation

* price per play: very low
* suggested initial range: `0.01 - 0.05` USDC equivalent

### Rationale

* highlights micropayment nature
* reduces user hesitation
* keeps the demo playful rather than high-stakes
* aligns with the machine-payment use case

---

## 21. Abuse / Edge Cases

The system must consider:

### Duplicate Retries

* same request resent because of network issues
* must return same result, not rerun

### Replay Attacks

* payment proof reused across games
* must be rejected

### Expired Payment Windows

* unpaid game session expires after a defined TTL

### Partial Settlement Failure

* if reward/capture step fails, system should remain recoverable
* game should not resolve multiple times during recovery

### Fairness Data Exposure

* reveal only after payment verification and resolution
* commit must exist before reveal

---

## 22. Risks

### Product Risk

The game may feel too simple and lose novelty quickly.

### Technical Risk

MPP Rust ecosystem may still be less mature than JS/TS tooling in some areas.

### Legal / Framing Risk

A paid game with win/loss outcomes can be interpreted as gambling-like if framed poorly.

### Trust Risk

If fairness proof is unclear, users may assume the server cheats.

---

## 23. Mitigations

### For Product Risk

* emphasize collectible rewards
* add inventory and leaderboard
* add streaks or seasonal goals in later versions

### For Technical Risk

* isolate MPP integration behind an adaptor layer
* keep business logic independent from protocol crate internals

### For Legal / Framing Risk

* avoid cash-equivalent token payouts
* avoid redeemable prize framing
* position rewards as collectible/non-cash utility items

### For Trust Risk

* provide visible fairness proof
* make verification reproducible and inspectable

---

## 24. Success Metrics

### MVP Success Metrics

* number of successful paid plays
* percentage of plays resolved without manual intervention
* payment verification success rate
* settlement success rate
* fairness verification page visits
* repeat play rate
* average plays per user

### Qualitative Metrics

* whether users understand MPP more clearly after using it
* whether the product is good enough to demo publicly
* whether the architecture is reusable for another paid endpoint project

---

## 25. Milestones

### Milestone 1 — Core Paid Play

* basic frontend
* `/play` endpoint
* `402 Payment Required` flow
* payment verification
* simple result response

### Milestone 2 — Settlement + Persistence

* game persistence
* settlement records
* inventory records
* receipts

### Milestone 3 — Fairness

* commit-reveal implementation
* fairness verification endpoint/page

### Milestone 4 — Product Polish

* leaderboard
* inventory UI
* better tracing/logging
* deployable Rust backend

### Milestone 5 — Optional v2

* onchain collectible rewards
* session-based repeated plays
* agent mode

---

## 26. Future Extensions

* ERC-1155 onchain collectibles
* session/prepaid play mode
* AI/agent autoplayer
* advanced pricing rules
* PvP or asynchronous player pools
* richer reward combinations
* public proof feed
* reusable paid-endpoint template extracted from project

---

## 27. Final Product Definition

Pay-per-Play RPS is a small but technically meaningful product that demonstrates how a service request can become a paid machine interaction.

Its core value is not “rock-paper-scissors” itself, but the combination of:

* paid HTTP access
* clean payment negotiation
* exact-once settlement
* fairness proof
* Rust backend architecture

This makes it a strong demo project for both:

* learning Rust backend development
* showcasing what MPP can enable in practice