ALTER TABLE aggregate_apis ADD COLUMN upstream_format TEXT NOT NULL DEFAULT 'responses';
ALTER TABLE aggregate_apis ADD COLUMN models_path TEXT;
ALTER TABLE aggregate_apis ADD COLUMN models_last_synced_at INTEGER;
ALTER TABLE aggregate_apis ADD COLUMN models_last_sync_status TEXT;
ALTER TABLE aggregate_apis ADD COLUMN models_last_sync_error TEXT;

CREATE TABLE IF NOT EXISTS aggregate_api_models (
  aggregate_api_id TEXT NOT NULL REFERENCES aggregate_apis(id) ON DELETE CASCADE,
  model_slug TEXT NOT NULL,
  display_name TEXT,
  raw_json TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (aggregate_api_id, model_slug)
);

CREATE INDEX IF NOT EXISTS idx_aggregate_api_models_updated_at
  ON aggregate_api_models(aggregate_api_id, updated_at DESC);
