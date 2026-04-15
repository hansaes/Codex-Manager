use crate::commands::shared::rpc_call_in_background;

#[tauri::command]
pub async fn service_team_list(addr: Option<String>) -> Result<serde_json::Value, String> {
    rpc_call_in_background("team/list", addr, None).await
}

#[tauri::command]
pub async fn service_team_add_from_account(
    addr: Option<String>,
    account_id: String,
) -> Result<serde_json::Value, String> {
    rpc_call_in_background(
        "team/addFromAccount",
        addr,
        Some(serde_json::json!({ "accountId": account_id })),
    )
    .await
}

#[tauri::command]
pub async fn service_team_sync(
    addr: Option<String>,
    team_id: String,
) -> Result<serde_json::Value, String> {
    rpc_call_in_background("team/sync", addr, Some(serde_json::json!({ "teamId": team_id }))).await
}

#[tauri::command]
pub async fn service_team_members(
    addr: Option<String>,
    team_id: String,
) -> Result<serde_json::Value, String> {
    rpc_call_in_background(
        "team/members",
        addr,
        Some(serde_json::json!({ "teamId": team_id })),
    )
    .await
}

#[tauri::command]
pub async fn service_team_invite(
    addr: Option<String>,
    team_id: String,
    emails: Vec<String>,
) -> Result<serde_json::Value, String> {
    rpc_call_in_background(
        "team/invite",
        addr,
        Some(serde_json::json!({ "teamId": team_id, "emails": emails })),
    )
    .await
}

#[tauri::command]
pub async fn service_team_delete(
    addr: Option<String>,
    team_id: String,
) -> Result<serde_json::Value, String> {
    rpc_call_in_background(
        "team/delete",
        addr,
        Some(serde_json::json!({ "teamId": team_id })),
    )
    .await
}
