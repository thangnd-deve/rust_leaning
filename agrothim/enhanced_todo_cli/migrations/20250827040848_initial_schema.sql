-- Add migration script here
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid (),
    username VARCHAR(50) NOT NULL UNIQUE,
    email VARCHAR(255) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    created_at TIMESTAMP
    WITH
        TIME ZONE DEFAULT CURRENT_TIMESTAMP,
        updated_at TIMESTAMP
    WITH
        TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_users_username ON users (username);

CREATE INDEX idx_users_email ON users (email);

CREATE INDEX idx_users_created_at ON users (created_at);

CREATE TABLE tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid (),
    title VARCHAR(255) NOT NULL,
    description TEXT,
    status SMALLINT NOT NULL DEFAULT 0 CONSTRAINT status_check CHECK (status IN (0, 1, 2)),
    priority SMALLINT NOT NULL DEFAULT 1 CONSTRAINT priority_check CHECK (priority IN (0, 1, 2)),
    due_date TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

COMMENT ON COLUMN tasks.status IS '0: Pending, 1: In Progress, 2: Completed';

COMMENT ON COLUMN tasks.priority IS '0: Low, 1: Medium, 2: High';

-- Indexes for efficient queries
CREATE INDEX idx_tasks_user_id ON tasks (user_id);

CREATE INDEX idx_tasks_status ON tasks (status);

CREATE INDEX idx_tasks_priority ON tasks (priority);

CREATE INDEX idx_tasks_due_date ON tasks (due_date)
WHERE
    due_date IS NOT NULL;

CREATE INDEX idx_tasks_created_at ON tasks (created_at);

-- Compound indexes for common query patterns
CREATE INDEX idx_tasks_user_status ON tasks (user_id, status);

CREATE INDEX idx_tasks_user_priority ON tasks (user_id, priority);