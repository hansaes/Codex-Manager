CREATE TABLE IF NOT EXISTS managed_teams (
  id TEXT PRIMARY KEY,
  source_account_id TEXT NOT NULL UNIQUE,
  team_account_id TEXT,
  team_name TEXT,
  plan_type TEXT,
  subscription_plan TEXT,
  status TEXT NOT NULL DEFAULT 'pending',
  current_members INTEGER NOT NULL DEFAULT 0,
  pending_invites INTEGER NOT NULL DEFAULT 0,
  max_members INTEGER NOT NULL DEFAULT 6,
  expires_at INTEGER,
  last_sync_at INTEGER,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_managed_teams_source_account_id
  ON managed_teams(source_account_id);

CREATE INDEX IF NOT EXISTS idx_managed_teams_team_account_id
  ON managed_teams(team_account_id);
