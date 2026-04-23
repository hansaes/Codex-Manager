use rusqlite::{params, Result, Row};

use super::{now_ts, AggregateApi, AggregateApiModel, Storage};

const AGGREGATE_API_SELECT_SQL: &str = "SELECT
    id,
    provider_type,
    supplier_name,
    sort,
    url,
    auth_type,
    auth_params_json,
    action,
    upstream_format,
    models_path,
    responses_path,
    chat_completions_path,
    proxy_mode,
    proxy_url,
    status,
    created_at,
    updated_at,
    last_test_at,
    last_test_status,
    last_test_error,
    models_last_synced_at,
    models_last_sync_status,
    models_last_sync_error
 FROM aggregate_apis";

impl Storage {
    /// 函数 `insert_aggregate_api`
    ///
    /// 作者: gaohongshun
    ///
    /// 时间: 2026-04-02
    ///
    /// # 参数
    /// - self: 参数 self
    /// - api: 参数 api
    ///
    /// # 返回
    /// 返回函数执行结果
    pub fn insert_aggregate_api(&self, api: &AggregateApi) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO aggregate_apis (
                id,
                provider_type,
                supplier_name,
                sort,
                url,
                auth_type,
                auth_params_json,
                action,
                upstream_format,
                models_path,
                responses_path,
                chat_completions_path,
                proxy_mode,
                proxy_url,
                status,
                created_at,
                updated_at,
                last_test_at,
                last_test_status,
                last_test_error,
                models_last_synced_at,
                models_last_sync_status,
                models_last_sync_error
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)",
            params![
                &api.id,
                &api.provider_type,
                &api.supplier_name,
                api.sort,
                &api.url,
                &api.auth_type,
                &api.auth_params_json,
                &api.action,
                &api.upstream_format,
                &api.models_path,
                &api.responses_path,
                &api.chat_completions_path,
                &api.proxy_mode,
                &api.proxy_url,
                &api.status,
                api.created_at,
                api.updated_at,
                &api.last_test_at,
                &api.last_test_status,
                &api.last_test_error,
                &api.models_last_synced_at,
                &api.models_last_sync_status,
                &api.models_last_sync_error,
            ],
        )?;
        Ok(())
    }

    /// 函数 `list_aggregate_apis`
    ///
    /// 作者: gaohongshun
    ///
    /// 时间: 2026-04-02
    ///
    /// # 参数
    /// - self: 参数 self
    ///
    /// # 返回
    /// 返回函数执行结果
    pub fn list_aggregate_apis(&self) -> Result<Vec<AggregateApi>> {
        let mut stmt = self.conn.prepare(&format!(
            "{AGGREGATE_API_SELECT_SQL} ORDER BY sort ASC, updated_at DESC"
        ))?;
        let mut rows = stmt.query([])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            out.push(map_aggregate_api_row(row)?);
        }
        Ok(out)
    }

    /// 函数 `find_aggregate_api_by_id`
    ///
    /// 作者: gaohongshun
    ///
    /// 时间: 2026-04-02
    ///
    /// # 参数
    /// - self: 参数 self
    /// - api_id: 参数 api_id
    ///
    /// # 返回
    /// 返回函数执行结果
    pub fn find_aggregate_api_by_id(&self, api_id: &str) -> Result<Option<AggregateApi>> {
        let mut stmt = self.conn.prepare(&format!(
            "{AGGREGATE_API_SELECT_SQL}
             WHERE id = ?1
             LIMIT 1"
        ))?;
        let mut rows = stmt.query([api_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(map_aggregate_api_row(row)?))
        } else {
            Ok(None)
        }
    }

    /// 函数 `update_aggregate_api`
    ///
    /// 作者: gaohongshun
    ///
    /// 时间: 2026-04-02
    ///
    /// # 参数
    /// - self: 参数 self
    /// - api_id: 参数 api_id
    /// - url: 参数 url
    ///
    /// # 返回
    /// 返回函数执行结果
    pub fn update_aggregate_api(&self, api_id: &str, url: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE aggregate_apis SET url = ?1, updated_at = ?2 WHERE id = ?3",
            (url, now_ts(), api_id),
        )?;
        Ok(())
    }

    /// 函数 `update_aggregate_api_supplier_name`
    ///
    /// 作者: gaohongshun
    ///
    /// 时间: 2026-04-02
    ///
    /// # 参数
    /// - self: 参数 self
    /// - api_id: 参数 api_id
    /// - supplier_name: 参数 supplier_name
    ///
    /// # 返回
    /// 返回函数执行结果
    pub fn update_aggregate_api_supplier_name(
        &self,
        api_id: &str,
        supplier_name: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE aggregate_apis SET supplier_name = ?1, updated_at = ?2 WHERE id = ?3",
            (supplier_name, now_ts(), api_id),
        )?;
        Ok(())
    }

    /// 函数 `update_aggregate_api_sort`
    ///
    /// 作者: gaohongshun
    ///
    /// 时间: 2026-04-02
    ///
    /// # 参数
    /// - self: 参数 self
    /// - api_id: 参数 api_id
    /// - sort: 参数 sort
    ///
    /// # 返回
    /// 返回函数执行结果
    pub fn update_aggregate_api_sort(&self, api_id: &str, sort: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE aggregate_apis SET sort = ?1, updated_at = ?2 WHERE id = ?3",
            (sort, now_ts(), api_id),
        )?;
        Ok(())
    }

    pub fn update_aggregate_api_status(&self, api_id: &str, status: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE aggregate_apis SET status = ?1, updated_at = ?2 WHERE id = ?3",
            (status, now_ts(), api_id),
        )?;
        Ok(())
    }

    /// 函数 `update_aggregate_api_type`
    ///
    /// 作者: gaohongshun
    ///
    /// 时间: 2026-04-02
    ///
    /// # 参数
    /// - self: 参数 self
    /// - api_id: 参数 api_id
    /// - provider_type: 参数 provider_type
    ///
    /// # 返回
    /// 返回函数执行结果
    pub fn update_aggregate_api_type(&self, api_id: &str, provider_type: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE aggregate_apis SET provider_type = ?1, updated_at = ?2 WHERE id = ?3",
            (provider_type, now_ts(), api_id),
        )?;
        Ok(())
    }

    pub fn update_aggregate_api_auth_type(&self, api_id: &str, auth_type: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE aggregate_apis SET auth_type = ?1, updated_at = ?2 WHERE id = ?3",
            (auth_type, now_ts(), api_id),
        )?;
        Ok(())
    }

    pub fn update_aggregate_api_auth_params_json(
        &self,
        api_id: &str,
        auth_params_json: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE aggregate_apis SET auth_params_json = ?1, updated_at = ?2 WHERE id = ?3",
            (auth_params_json, now_ts(), api_id),
        )?;
        Ok(())
    }

    pub fn update_aggregate_api_action(&self, api_id: &str, action: Option<&str>) -> Result<()> {
        self.conn.execute(
            "UPDATE aggregate_apis SET action = ?1, updated_at = ?2 WHERE id = ?3",
            (action, now_ts(), api_id),
        )?;
        Ok(())
    }

    pub fn update_aggregate_api_upstream_format(
        &self,
        api_id: &str,
        upstream_format: &str,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE aggregate_apis SET upstream_format = ?1, updated_at = ?2 WHERE id = ?3",
            (upstream_format, now_ts(), api_id),
        )?;
        Ok(())
    }

    pub fn update_aggregate_api_models_path(
        &self,
        api_id: &str,
        models_path: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE aggregate_apis SET models_path = ?1, updated_at = ?2 WHERE id = ?3",
            (models_path, now_ts(), api_id),
        )?;
        Ok(())
    }

    pub fn update_aggregate_api_responses_path(
        &self,
        api_id: &str,
        responses_path: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE aggregate_apis SET responses_path = ?1, updated_at = ?2 WHERE id = ?3",
            (responses_path, now_ts(), api_id),
        )?;
        Ok(())
    }

    pub fn update_aggregate_api_chat_completions_path(
        &self,
        api_id: &str,
        chat_completions_path: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE aggregate_apis SET chat_completions_path = ?1, updated_at = ?2 WHERE id = ?3",
            (chat_completions_path, now_ts(), api_id),
        )?;
        Ok(())
    }

    pub fn update_aggregate_api_proxy_mode(&self, api_id: &str, proxy_mode: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE aggregate_apis SET proxy_mode = ?1, updated_at = ?2 WHERE id = ?3",
            (proxy_mode, now_ts(), api_id),
        )?;
        Ok(())
    }

    pub fn update_aggregate_api_proxy_url(
        &self,
        api_id: &str,
        proxy_url: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE aggregate_apis SET proxy_url = ?1, updated_at = ?2 WHERE id = ?3",
            (proxy_url, now_ts(), api_id),
        )?;
        Ok(())
    }

    pub fn update_aggregate_api_models_sync_result(
        &self,
        api_id: &str,
        synced_at: Option<i64>,
        sync_status: Option<&str>,
        sync_error: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE aggregate_apis
             SET models_last_synced_at = ?1,
                 models_last_sync_status = ?2,
                 models_last_sync_error = ?3,
                 updated_at = ?4
             WHERE id = ?5",
            (synced_at, sync_status, sync_error, now_ts(), api_id),
        )?;
        Ok(())
    }

    pub fn replace_aggregate_api_models(
        &self,
        aggregate_api_id: &str,
        models: &[AggregateApiModel],
    ) -> Result<()> {
        self.conn.execute(
            "DELETE FROM aggregate_api_models WHERE aggregate_api_id = ?1",
            [aggregate_api_id],
        )?;
        for model in models {
            self.conn.execute(
                "INSERT INTO aggregate_api_models (
                    aggregate_api_id,
                    model_slug,
                    display_name,
                    raw_json,
                    created_at,
                    updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    model.aggregate_api_id,
                    model.model_slug,
                    model.display_name,
                    model.raw_json,
                    model.created_at,
                    model.updated_at
                ],
            )?;
        }
        Ok(())
    }

    pub fn list_aggregate_api_models(
        &self,
        aggregate_api_id: &str,
    ) -> Result<Vec<AggregateApiModel>> {
        let mut stmt = self.conn.prepare(
            "SELECT
                aggregate_api_id,
                model_slug,
                display_name,
                raw_json,
                created_at,
                updated_at
             FROM aggregate_api_models
             WHERE aggregate_api_id = ?1
             ORDER BY model_slug ASC",
        )?;
        let mut rows = stmt.query([aggregate_api_id])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            out.push(AggregateApiModel {
                aggregate_api_id: row.get(0)?,
                model_slug: row.get(1)?,
                display_name: row.get(2)?,
                raw_json: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            });
        }
        Ok(out)
    }

    /// 函数 `delete_aggregate_api`
    ///
    /// 作者: gaohongshun
    ///
    /// 时间: 2026-04-02
    ///
    /// # 参数
    /// - self: 参数 self
    /// - api_id: 参数 api_id
    ///
    /// # 返回
    /// 返回函数执行结果
    pub fn delete_aggregate_api(&self, api_id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM aggregate_api_models WHERE aggregate_api_id = ?1",
            [api_id],
        )?;
        self.conn.execute(
            "DELETE FROM aggregate_api_secrets WHERE aggregate_api_id = ?1",
            [api_id],
        )?;
        self.conn
            .execute("DELETE FROM aggregate_apis WHERE id = ?1", [api_id])?;
        Ok(())
    }

    /// 函数 `upsert_aggregate_api_secret`
    ///
    /// 作者: gaohongshun
    ///
    /// 时间: 2026-04-02
    ///
    /// # 参数
    /// - self: 参数 self
    /// - api_id: 参数 api_id
    /// - secret_value: 参数 secret_value
    ///
    /// # 返回
    /// 返回函数执行结果
    pub fn upsert_aggregate_api_secret(&self, api_id: &str, secret_value: &str) -> Result<()> {
        let now = now_ts();
        self.conn.execute(
            "INSERT INTO aggregate_api_secrets (aggregate_api_id, secret_value, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?3)
             ON CONFLICT(aggregate_api_id) DO UPDATE SET
               secret_value = excluded.secret_value,
               updated_at = excluded.updated_at",
            (api_id, secret_value, now),
        )?;
        Ok(())
    }

    /// 函数 `find_aggregate_api_secret_by_id`
    ///
    /// 作者: gaohongshun
    ///
    /// 时间: 2026-04-02
    ///
    /// # 参数
    /// - self: 参数 self
    /// - api_id: 参数 api_id
    ///
    /// # 返回
    /// 返回函数执行结果
    pub fn find_aggregate_api_secret_by_id(&self, api_id: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT secret_value FROM aggregate_api_secrets WHERE aggregate_api_id = ?1 LIMIT 1",
        )?;
        let mut rows = stmt.query([api_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    /// 函数 `update_aggregate_api_test_result`
    ///
    /// 作者: gaohongshun
    ///
    /// 时间: 2026-04-02
    ///
    /// # 参数
    /// - self: 参数 self
    /// - api_id: 参数 api_id
    /// - ok: 参数 ok
    /// - status_code: 参数 status_code
    /// - error: 参数 error
    ///
    /// # 返回
    /// 返回函数执行结果
    pub fn update_aggregate_api_test_result(
        &self,
        api_id: &str,
        ok: bool,
        status_code: Option<i64>,
        error: Option<&str>,
    ) -> Result<()> {
        let now = now_ts();
        let last_test_status = if ok { Some("success") } else { Some("failed") };
        self.conn.execute(
            "UPDATE aggregate_apis
             SET last_test_at = ?1,
                 last_test_status = ?2,
                 last_test_error = ?3,
                 updated_at = ?1
             WHERE id = ?4",
            (now, last_test_status, error, api_id),
        )?;
        if let Some(code) = status_code {
            if !ok {
                let message = format!("http_status={code}");
                self.conn.execute(
                    "UPDATE aggregate_apis SET last_test_error = ?1 WHERE id = ?2",
                    (message, api_id),
                )?;
            }
        }
        Ok(())
    }

    /// 函数 `ensure_aggregate_apis_table`
    ///
    /// 作者: gaohongshun
    ///
    /// 时间: 2026-04-02
    ///
    /// # 参数
    /// - super: 参数 super
    ///
    /// # 返回
    /// 返回函数执行结果
    pub(super) fn ensure_aggregate_apis_table(&self) -> Result<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS aggregate_apis (
                id TEXT PRIMARY KEY,
                provider_type TEXT NOT NULL DEFAULT 'codex',
                supplier_name TEXT,
                sort INTEGER NOT NULL DEFAULT 0,
                url TEXT NOT NULL,
                auth_type TEXT NOT NULL DEFAULT 'apikey',
                auth_params_json TEXT,
                action TEXT,
                upstream_format TEXT NOT NULL DEFAULT 'responses',
                models_path TEXT,
                responses_path TEXT,
                chat_completions_path TEXT,
                proxy_mode TEXT NOT NULL DEFAULT 'follow_global',
                proxy_url TEXT,
                status TEXT NOT NULL DEFAULT 'active',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                last_test_at INTEGER,
                last_test_status TEXT,
                last_test_error TEXT,
                models_last_synced_at INTEGER,
                models_last_sync_status TEXT,
                models_last_sync_error TEXT
            )",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_aggregate_apis_created_at ON aggregate_apis(created_at DESC)",
            [],
        )?;
        self.ensure_column("aggregate_apis", "provider_type", "TEXT")?;
        self.ensure_column("aggregate_apis", "supplier_name", "TEXT")?;
        self.ensure_column("aggregate_apis", "sort", "INTEGER DEFAULT 0")?;
        self.ensure_column(
            "aggregate_apis",
            "auth_type",
            "TEXT NOT NULL DEFAULT 'apikey'",
        )?;
        self.ensure_column("aggregate_apis", "auth_params_json", "TEXT")?;
        self.ensure_column("aggregate_apis", "action", "TEXT")?;
        self.ensure_column(
            "aggregate_apis",
            "upstream_format",
            "TEXT NOT NULL DEFAULT 'responses'",
        )?;
        self.ensure_column("aggregate_apis", "models_path", "TEXT")?;
        self.ensure_column("aggregate_apis", "responses_path", "TEXT")?;
        self.ensure_column("aggregate_apis", "chat_completions_path", "TEXT")?;
        self.ensure_column(
            "aggregate_apis",
            "proxy_mode",
            "TEXT NOT NULL DEFAULT 'follow_global'",
        )?;
        self.ensure_column("aggregate_apis", "proxy_url", "TEXT")?;
        self.ensure_column("aggregate_apis", "models_last_synced_at", "INTEGER")?;
        self.ensure_column("aggregate_apis", "models_last_sync_status", "TEXT")?;
        self.ensure_column("aggregate_apis", "models_last_sync_error", "TEXT")?;
        self.conn.execute(
            "UPDATE aggregate_apis
             SET provider_type = COALESCE(NULLIF(TRIM(provider_type), ''), 'codex')
             WHERE provider_type IS NULL OR TRIM(provider_type) = ''",
            [],
        )?;
        self.conn.execute(
            "UPDATE aggregate_apis
             SET auth_type = COALESCE(NULLIF(TRIM(auth_type), ''), 'apikey')
             WHERE auth_type IS NULL OR TRIM(auth_type) = ''",
            [],
        )?;
        self.conn.execute(
            "UPDATE aggregate_apis
             SET sort = COALESCE(sort, 0)
             WHERE sort IS NULL",
            [],
        )?;
        self.conn.execute(
            "UPDATE aggregate_apis
             SET upstream_format = COALESCE(NULLIF(TRIM(upstream_format), ''), 'responses')
             WHERE upstream_format IS NULL OR TRIM(upstream_format) = ''",
            [],
        )?;
        self.conn.execute(
            "UPDATE aggregate_apis
             SET proxy_mode = COALESCE(NULLIF(TRIM(proxy_mode), ''), 'follow_global')
             WHERE proxy_mode IS NULL OR TRIM(proxy_mode) = ''",
            [],
        )?;
        Ok(())
    }

    /// 函数 `ensure_aggregate_api_secrets_table`
    ///
    /// 作者: gaohongshun
    ///
    /// 时间: 2026-04-02
    ///
    /// # 参数
    /// - super: 参数 super
    ///
    /// # 返回
    /// 返回函数执行结果
    pub(super) fn ensure_aggregate_api_secrets_table(&self) -> Result<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS aggregate_api_secrets (
                aggregate_api_id TEXT PRIMARY KEY REFERENCES aggregate_apis(id) ON DELETE CASCADE,
                secret_value TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_aggregate_api_secrets_updated_at ON aggregate_api_secrets(updated_at)",
            [],
        )?;
        Ok(())
    }

    pub(super) fn ensure_aggregate_api_models_table(&self) -> Result<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS aggregate_api_models (
                aggregate_api_id TEXT NOT NULL REFERENCES aggregate_apis(id) ON DELETE CASCADE,
                model_slug TEXT NOT NULL,
                display_name TEXT,
                raw_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (aggregate_api_id, model_slug)
            )",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_aggregate_api_models_updated_at
             ON aggregate_api_models(aggregate_api_id, updated_at DESC)",
            [],
        )?;
        Ok(())
    }
}

/// 函数 `map_aggregate_api_row`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - row: 参数 row
///
/// # 返回
/// 返回函数执行结果
fn map_aggregate_api_row(row: &Row<'_>) -> Result<AggregateApi> {
    Ok(AggregateApi {
        id: row.get(0)?,
        provider_type: row.get(1)?,
        supplier_name: row.get(2)?,
        sort: row.get(3)?,
        url: row.get(4)?,
        auth_type: row.get(5)?,
        auth_params_json: row.get(6)?,
        action: row.get(7)?,
        upstream_format: row.get(8)?,
        models_path: row.get(9)?,
        responses_path: row.get(10)?,
        chat_completions_path: row.get(11)?,
        proxy_mode: row.get(12)?,
        proxy_url: row.get(13)?,
        status: row.get(14)?,
        created_at: row.get(15)?,
        updated_at: row.get(16)?,
        last_test_at: row.get(17)?,
        last_test_status: row.get(18)?,
        last_test_error: row.get(19)?,
        models_last_synced_at: row.get(20)?,
        models_last_sync_status: row.get(21)?,
        models_last_sync_error: row.get(22)?,
    })
}
