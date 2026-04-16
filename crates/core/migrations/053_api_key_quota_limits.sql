CREATE TABLE IF NOT EXISTS api_key_quota_limits (
  key_id TEXT PRIMARY KEY REFERENCES api_keys(id) ON DELETE CASCADE,
  total_token_limit INTEGER,
  total_cost_usd_limit REAL,
  total_request_limit INTEGER
);
