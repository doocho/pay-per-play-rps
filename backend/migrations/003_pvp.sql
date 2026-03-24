-- PvP game status state machine
CREATE TYPE pvp_game_status AS ENUM (
    'waiting_for_opponent',
    'player1_paid',
    'player2_paid',
    'both_paid',
    'player1_committed',
    'player2_committed',
    'both_committed',
    'player1_revealed',
    'player2_revealed',
    'resolved_player1_wins',
    'resolved_player2_wins',
    'resolved_draw',
    'settling',
    'settled',
    'expired',
    'cancelled'
);

CREATE TYPE pvp_outcome AS ENUM ('player1_wins', 'player2_wins', 'draw');

-- PvP games table
CREATE TABLE pvp_games (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Room / matchmaking
    room_code TEXT UNIQUE,

    -- Players
    player1_id UUID REFERENCES users(id),
    player2_id UUID REFERENCES users(id),

    -- Game config
    status pvp_game_status NOT NULL DEFAULT 'waiting_for_opponent',
    price NUMERIC(20, 6) NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    platform_fee_bps INTEGER NOT NULL DEFAULT 500,

    -- Player 1 commit-reveal
    player1_choice choice,
    player1_salt TEXT,
    player1_commit TEXT,

    -- Player 2 commit-reveal
    player2_choice choice,
    player2_salt TEXT,
    player2_commit TEXT,

    -- Result
    result pvp_outcome,
    rounds JSONB NOT NULL DEFAULT '[]',
    current_round INTEGER NOT NULL DEFAULT 1,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    player2_joined_at TIMESTAMPTZ,
    both_paid_at TIMESTAMPTZ,
    both_committed_at TIMESTAMPTZ,
    resolved_at TIMESTAMPTZ,
    settled_at TIMESTAMPTZ
);

-- PvP payments: one per player per game
CREATE TABLE pvp_payments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    pvp_game_id UUID NOT NULL REFERENCES pvp_games(id),
    player_id UUID NOT NULL REFERENCES users(id),
    amount NUMERIC(20, 6) NOT NULL,
    protocol TEXT NOT NULL DEFAULT 'mpp',
    network TEXT NOT NULL DEFAULT 'tempo',
    asset TEXT NOT NULL DEFAULT 'USDC',
    provider_payment_id TEXT,
    authorization_payload JSONB,
    receipt_payload JSONB,
    status TEXT NOT NULL DEFAULT 'authorized',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (pvp_game_id, player_id)
);

-- PvP settlements
CREATE TABLE pvp_settlements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    pvp_game_id UUID NOT NULL UNIQUE REFERENCES pvp_games(id),
    result pvp_outcome NOT NULL,
    pot_amount NUMERIC(20, 6) NOT NULL,
    platform_fee NUMERIC(20, 6) NOT NULL,
    winner_payout NUMERIC(20, 6) NOT NULL,
    loser_refund NUMERIC(20, 6) NOT NULL DEFAULT 0,
    winner_id UUID REFERENCES users(id),
    reward_token token_type,
    reward_amount INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'completed',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Matchmaking queue
CREATE TABLE matchmaking_queue (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    pvp_game_id UUID NOT NULL REFERENCES pvp_games(id),
    price NUMERIC(20, 6) NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    enqueued_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id)
);

-- Indexes
CREATE INDEX idx_pvp_games_status ON pvp_games (status);
CREATE INDEX idx_pvp_games_room_code ON pvp_games (room_code) WHERE room_code IS NOT NULL;
CREATE INDEX idx_pvp_games_waiting ON pvp_games (status) WHERE status = 'waiting_for_opponent';
CREATE INDEX idx_pvp_games_player1 ON pvp_games (player1_id, created_at DESC);
CREATE INDEX idx_pvp_games_player2 ON pvp_games (player2_id, created_at DESC);
CREATE INDEX idx_matchmaking_queue_price ON matchmaking_queue (price, currency, enqueued_at);
CREATE UNIQUE INDEX idx_pvp_payments_provider_id ON pvp_payments (provider_payment_id) WHERE provider_payment_id IS NOT NULL;
