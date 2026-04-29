use codexmanager_core::rpc::types::{JsonRpcRequest, JsonRpcResponse};

use crate::team_management;

pub(super) fn try_handle(req: &JsonRpcRequest) -> Option<JsonRpcResponse> {
    let result = match req.method.as_str() {
        "team/list" => super::value_or_error(team_management::list_managed_teams()),
        "team/addFromAccount" => {
            let account_id = super::str_param(req, "accountId").unwrap_or("");
            super::value_or_error(team_management::add_managed_team_from_account(account_id))
        }
        "team/sync" => {
            let team_id = super::str_param(req, "teamId").unwrap_or("");
            super::value_or_error(team_management::sync_managed_team(team_id))
        }
        "team/members" => {
            let team_id = super::str_param(req, "teamId").unwrap_or("");
            super::value_or_error(team_management::list_managed_team_members(team_id))
        }
        "team/invite" => {
            let team_id = super::str_param(req, "teamId").unwrap_or("");
            let emails = req
                .params
                .as_ref()
                .and_then(|params| params.get("emails"))
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str())
                        .map(|item| item.to_string())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            super::value_or_error(team_management::invite_managed_team_members(
                team_id, emails,
            ))
        }
        "team/removeMember" => {
            let team_id = super::str_param(req, "teamId").unwrap_or("");
            let user_id = super::str_param(req, "userId").unwrap_or("");
            super::value_or_error(team_management::remove_managed_team_member(
                team_id, user_id,
            ))
        }
        "team/revokeInvite" => {
            let team_id = super::str_param(req, "teamId").unwrap_or("");
            let email = super::str_param(req, "email").unwrap_or("");
            super::value_or_error(team_management::revoke_managed_team_invite(team_id, email))
        }
        "team/delete" => {
            let team_id = super::str_param(req, "teamId").unwrap_or("");
            super::ok_or_error(team_management::delete_managed_team(team_id))
        }
        _ => return None,
    };

    Some(super::response(req, result))
}
