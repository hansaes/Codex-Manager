CREATE TABLE IF NOT EXISTS gateway_error_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    trace_id TEXT,
    key_id TEXT,
    account_id TEXT,
    request_path TEXT NOT NULL,
    method TEXT NOT NULL,
    stage TEXT NOT NULL,
    error_kind TEXT,
    upstream_url TEXT,
    cf_ray TEXT,
    status_code INTEGER,
    compression_enabled INTEGER NOT NULL DEFAULT 0,
    compression_retry_attempted INTEGER NOT NULL DEFAULT 0,
    message TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_gateway_error_logs_created_at
ON gateway_error_logs(created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_gateway_error_logs_trace_id
ON gateway_error_logs(trace_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_gateway_error_logs_stage
ON gateway_error_logs(stage, created_at DESC);
