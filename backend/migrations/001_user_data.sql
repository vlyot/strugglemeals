-- Phase 3: Auth & User Data
-- Run against Neon Postgres (already applied)

CREATE TABLE IF NOT EXISTS cook_history (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     TEXT        NOT NULL,
    recipe_id   BIGINT      NOT NULL,
    recipe_name TEXT        NOT NULL,
    cooked_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS cook_history_user_id_cooked_at_idx ON cook_history (user_id, cooked_at DESC);

CREATE TABLE IF NOT EXISTS favourites (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     TEXT        NOT NULL,
    recipe_id   BIGINT      NOT NULL,
    recipe_name TEXT        NOT NULL,
    saved_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT favourites_user_recipe_unique UNIQUE (user_id, recipe_id)
);
CREATE INDEX IF NOT EXISTS favourites_user_id_saved_at_idx ON favourites (user_id, saved_at DESC);
