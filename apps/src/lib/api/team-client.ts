import { invoke, withAddr } from "./transport";
import {
  normalizeManagedTeam,
  normalizeManagedTeamInviteResult,
  normalizeManagedTeamList,
  normalizeManagedTeamMembersResult,
} from "./normalize";
import {
  ManagedTeam,
  ManagedTeamInviteResult,
  ManagedTeamMembersResult,
} from "../../types";

export const teamClient = {
  async list(): Promise<ManagedTeam[]> {
    const result = await invoke<unknown>("service_team_list", withAddr());
    return normalizeManagedTeamList(result);
  },
  async addFromAccount(accountId: string): Promise<ManagedTeam> {
    const result = await invoke<unknown>(
      "service_team_add_from_account",
      withAddr({ accountId }),
    );
    const normalized = normalizeManagedTeam(result);
    if (!normalized) {
      throw new Error("团队母号添加结果为空");
    }
    return normalized;
  },
  async sync(teamId: string): Promise<ManagedTeam> {
    const result = await invoke<unknown>(
      "service_team_sync",
      withAddr({ teamId }),
    );
    const normalized = normalizeManagedTeam(result);
    if (!normalized) {
      throw new Error("团队同步结果为空");
    }
    return normalized;
  },
  async members(teamId: string): Promise<ManagedTeamMembersResult> {
    const result = await invoke<unknown>(
      "service_team_members",
      withAddr({ teamId }),
    );
    return normalizeManagedTeamMembersResult(result);
  },
  async invite(teamId: string, emails: string[]): Promise<ManagedTeamInviteResult> {
    const result = await invoke<unknown>(
      "service_team_invite",
      withAddr({ teamId, emails }),
    );
    return normalizeManagedTeamInviteResult(result);
  },
  delete(teamId: string) {
    return invoke("service_team_delete", withAddr({ teamId }));
  },
};
