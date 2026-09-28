-- API key store.
--
-- There is intentionally no user table: the single administrator is authenticated
-- out-of-band via the `ADMIN_API_KEY` environment variable. Each row is one API
-- key with a consumable workflow quota and an associated vector store URL.
--
-- Keys are never stored in plaintext: only their SHA-256 hash (hex) is kept, and
-- a short human-readable prefix is retained for display. The plaintext key is
-- returned to the caller exactly once, at creation time.
CREATE TABLE api_keys (
    id               UUID        PRIMARY KEY,
    key_prefix       TEXT        NOT NULL,
    key_hash         TEXT        NOT NULL UNIQUE,
    name             TEXT,
    workflow_limit   BIGINT      NOT NULL DEFAULT 0 CHECK (workflow_limit >= 0),
    workflow_used    BIGINT      NOT NULL DEFAULT 0 CHECK (workflow_used  >= 0),
    vector_store_url TEXT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- The UNIQUE constraint on `key_hash` already provides the index used by the
-- authentication lookup (`WHERE key_hash = $1`), so no extra index is required.
