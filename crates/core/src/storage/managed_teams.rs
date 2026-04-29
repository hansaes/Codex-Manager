use rusqlite::Result;

use super::{now_ts, Account, ManagedTeam, Storage};

impl Storage {
    pub fn insert_managed_team(&self, team: &ManagedTeam) -> Result<()> {
        self.conn.execute(
            "INSERT INTO managed_teams (
                id,
                source_account_id,
                team_account_id,
                team_name,
                plan_type,
                subscription_plan,
                status,
                current_members,
                pending_invites,
                max_members,
                expires_at,
                last_sync_at,
                created_at,
                updated_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14
            )
            ON CONFLICT(id) DO UPDATE SET
                source_account_id = excluded.source_account_id,
                team_account_id = excluded.team_account_id,
                team_name = excluded.team_name,
                plan_type = excluded.plan_type,
                subscription_plan = excluded.subscription_plan,
                status = excluded.status,
                current_members = excluded.current_members,
                pending_invites = excluded.pending_invites,
                max_members = excluded.max_members,
                expires_at = excluded.expires_at,
                last_sync_at = excluded.last_sync_at,
                updated_at = excluded.updated_at",
            (
                &team.id,
                &team.source_account_id,
                &team.team_account_id,
                &team.team_name,
                &team.plan_type,
                &team.subscription_plan,
                &team.status,
                team.current_members,
                team.pending_invites,
                team.max_members,
                team.expires_at,
                team.last_sync_at,
                team.created_at,
                team.updated_at,
            ),
        )?;
        Ok(())
    }

    pub fn list_managed_teams(&self) -> Result<Vec<(ManagedTeam, Option<Account>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT
                mt.id,
                mt.source_account_id,
                mt.team_account_id,
                mt.team_name,
                mt.plan_type,
                mt.subscription_plan,
                mt.status,
                mt.current_members,
                mt.pending_invites,
                mt.max_members,
                mt.expires_at,
                mt.last_sync_at,
                mt.created_at,
                mt.updated_at,
                a.id,
                a.label,
                a.issuer,
                a.chatgpt_account_id,
                a.workspace_id,
                a.sort,
                a.status,
                a.created_at,
                a.updated_at
             FROM managed_teams mt
             LEFT JOIN accounts a
               ON a.id = mt.source_account_id
             ORDER BY COALESCE(a.sort, 9223372036854775807) ASC, mt.updated_at DESC, mt.id ASC",
        )?;
        let mut rows = stmt.query([])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            let team = ManagedTeam {
                id: row.get(0)?,
                source_account_id: row.get(1)?,
                team_account_id: row.get(2)?,
                team_name: row.get(3)?,
                plan_type: row.get(4)?,
                subscription_plan: row.get(5)?,
                status: row.get(6)?,
                current_members: row.get(7)?,
                pending_invites: row.get(8)?,
                max_members: row.get(9)?,
                expires_at: row.get(10)?,
                last_sync_at: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            };
            let account = row.get::<_, Option<String>>(14)?.map(|id| Account {
                id,
                label: row.get(15).unwrap_or_default(),
                issuer: row.get(16).unwrap_or_default(),
                chatgpt_account_id: row.get(17).unwrap_or(None),
                workspace_id: row.get(18).unwrap_or(None),
                group_name: None,
                sort: row.get(19).unwrap_or(0),
                status: row.get(20).unwrap_or_else(|_| "unknown".to_string()),
                created_at: row.get(21).unwrap_or(0),
                updated_at: row.get(22).unwrap_or(0),
            });
            out.push((team, account));
        }
        Ok(out)
    }

    pub fn find_managed_team_by_id(&self, team_id: &str) -> Result<Option<ManagedTeam>> {
        let mut stmt = self.conn.prepare(
            "SELECT
                id,
                source_account_id,
                team_account_id,
                team_name,
                plan_type,
                subscription_plan,
                status,
                current_members,
                pending_invites,
                max_members,
                expires_at,
                last_sync_at,
                created_at,
                updated_at
             FROM managed_teams
             WHERE id = ?1
             LIMIT 1",
        )?;
        let mut rows = stmt.query([team_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(ManagedTeam {
                id: row.get(0)?,
                source_account_id: row.get(1)?,
                team_account_id: row.get(2)?,
                team_name: row.get(3)?,
                plan_type: row.get(4)?,
                subscription_plan: row.get(5)?,
                status: row.get(6)?,
                current_members: row.get(7)?,
                pending_invites: row.get(8)?,
                max_members: row.get(9)?,
                expires_at: row.get(10)?,
                last_sync_at: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn delete_managed_team(&self, team_id: &str) -> Result<bool> {
        let deleted = self
            .conn
            .execute("DELETE FROM managed_teams WHERE id = ?1", [team_id])?;
        Ok(deleted > 0)
    }

    pub fn create_managed_team_placeholder(&self, source_account_id: &str) -> Result<ManagedTeam> {
        let now = now_ts();
        let team = ManagedTeam {
            id: source_account_id.trim().to_string(),
            source_account_id: source_account_id.trim().to_string(),
            team_account_id: None,
            team_name: None,
            plan_type: None,
            subscription_plan: None,
            status: "pending".to_string(),
            current_members: 0,
            pending_invites: 0,
            max_members: 6,
            expires_at: None,
            last_sync_at: None,
            created_at: now,
            updated_at: now,
        };
        self.insert_managed_team(&team)?;
        Ok(team)
    }
}
