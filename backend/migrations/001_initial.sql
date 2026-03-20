-- Custom enum types
CREATE TYPE game_status AS ENUM (
    'created',
    'payment_required',
    'payment_authorized',
    'play_locked',
    'resolved_win',
    'resolved_draw',
    'resolved_lose',
    'settling',
    'settled',
    'expired',
    'failed'
);

CREATE TYPE choice AS ENUM ('rock', 'paper', 'scissors');
CREATE TYPE outcome AS ENUM ('win', 'draw', 'lose');
CREATE TYPE token_type AS ENUM ('rock', 'paper', 'scissors');

-- Users table
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_address TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Games table
CREATE TABLE games (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id),  -- nullable until payment verified
    status game_status NOT NULL DEFAULT 'created',
    price NUMERIC(20, 6) NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    user_choice choice NOT NULL,
    server_choice choice NOT NULL,
    server_salt TEXT NOT NULL,
    server_commit TEXT NOT NULL,
    result outcome,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at TIMESTAMPTZ,
    settled_at TIMESTAMPTZ
);

-- Payments table
CREATE TABLE payments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    game_id UUID NOT NULL UNIQUE REFERENCES games(id),
    protocol TEXT NOT NULL DEFAULT 'mpp',
    network TEXT NOT NULL DEFAULT 'tempo',
    asset TEXT NOT NULL DEFAULT 'USDC',
    amount NUMERIC(20, 6) NOT NULL,
    status TEXT NOT NULL DEFAULT 'authorized',
    provider_payment_id TEXT,
    authorization_payload JSONB,
    receipt_payload JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Settlements table
CREATE TABLE settlements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    game_id UUID NOT NULL UNIQUE REFERENCES games(id),
    outcome outcome NOT NULL,
    refund_amount NUMERIC(20, 6) NOT NULL DEFAULT 0,
    captured_amount NUMERIC(20, 6) NOT NULL DEFAULT 0,
    reward_token token_type,
    reward_amount INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'completed',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Inventories table
CREATE TABLE inventories (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    token_type token_type NOT NULL,
    balance INTEGER NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, token_type)
);

-- Idempotency table
CREATE TABLE idempotency (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key TEXT NOT NULL UNIQUE,
    scope TEXT,
    response_status INTEGER NOT NULL,
    response_body JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL
);

-- Indexes
CREATE INDEX idx_games_user_history ON games (user_id, created_at DESC);
CREATE INDEX idx_games_payment_required ON games (status) WHERE status = 'payment_required';
CREATE UNIQUE INDEX idx_payments_provider_id ON payments (provider_payment_id) WHERE provider_payment_id IS NOT NULL;
CREATE INDEX idx_idempotency_expires ON idempotency (expires_at);
