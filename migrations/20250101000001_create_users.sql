-- Create users table
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE users (
    user_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    username VARCHAR(50) UNIQUE NOT NULL,
    display_name VARCHAR(100),
    bio TEXT,
    avatar_url TEXT,
    btc_withdrawal_address VARCHAR(100),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    kyc_status VARCHAR(20) NOT NULL DEFAULT 'pending',
    reputation_score NUMERIC(10, 2) NOT NULL DEFAULT 500.0,
    role VARCHAR(20) NOT NULL DEFAULT 'user'
);

CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_reputation ON users(reputation_score);
