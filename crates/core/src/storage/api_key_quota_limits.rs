use rusqlite::Result;

use super::{ApiKeyQuotaLimits, Storage};

impl Storage {
    pub fn set_api_key_quota_limits(
        &self,
        key_id: &str,
        total_token_limit: Option<i64>,
        total_cost_usd_limit: Option<f64>,
        total_request_limit: Option<i64>,
    ) -> Result<()> {
        if total_token_limit.is_none()
            && total_cost_usd_limit.is_none()
            && total_request_limit.is_none()
        {
            self.conn.execute(
                "DELETE FROM api_key_quota_limits WHERE key_id = ?1",
                [key_id],
            )?;
            return Ok(());
        }

        self.conn.execute(
            "INSERT INTO api_key_quota_limits (
                key_id,
                total_token_limit,
                total_cost_usd_limit,
                total_request_limit
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(key_id) DO UPDATE SET
                total_token_limit = excluded.total_token_limit,
                total_cost_usd_limit = excluded.total_cost_usd_limit,
                total_request_limit = excluded.total_request_limit",
            (
                key_id,
                total_token_limit,
                total_cost_usd_limit,
                total_request_limit,
            ),
        )?;
        Ok(())
    }

    pub fn find_api_key_quota_limits(&self, key_id: &str) -> Result<Option<ApiKeyQuotaLimits>> {
        let mut stmt = self.conn.prepare(
            "SELECT
                key_id,
                total_token_limit,
                total_cost_usd_limit,
                total_request_limit
             FROM api_key_quota_limits
             WHERE key_id = ?1
             LIMIT 1",
        )?;
        let mut rows = stmt.query([key_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(ApiKeyQuotaLimits {
                key_id: row.get(0)?,
                total_token_limit: row.get(1)?,
                total_cost_usd_limit: row.get(2)?,
                total_request_limit: row.get(3)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn list_api_key_quota_limits(&self) -> Result<Vec<ApiKeyQuotaLimits>> {
        let mut stmt = self.conn.prepare(
            "SELECT
                key_id,
                total_token_limit,
                total_cost_usd_limit,
                total_request_limit
             FROM api_key_quota_limits
             ORDER BY key_id ASC",
        )?;
        let mut rows = stmt.query([])?;
        let mut items = Vec::new();
        while let Some(row) = rows.next()? {
            items.push(ApiKeyQuotaLimits {
                key_id: row.get(0)?,
                total_token_limit: row.get(1)?,
                total_cost_usd_limit: row.get(2)?,
                total_request_limit: row.get(3)?,
            });
        }
        Ok(items)
    }

    pub(super) fn ensure_api_key_quota_limits_table(&self) -> Result<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS api_key_quota_limits (
                key_id TEXT PRIMARY KEY REFERENCES api_keys(id) ON DELETE CASCADE,
                total_token_limit INTEGER,
                total_cost_usd_limit REAL,
                total_request_limit INTEGER
            )",
            [],
        )?;
        Ok(())
    }
}
