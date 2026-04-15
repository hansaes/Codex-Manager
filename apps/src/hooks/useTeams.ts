"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { useDeferredDesktopActivation } from "@/hooks/useDeferredDesktopActivation";
import { useDesktopPageActive } from "@/hooks/useDesktopPageActive";
import { useRuntimeCapabilities } from "@/hooks/useRuntimeCapabilities";
import { useI18n } from "@/lib/i18n/provider";
import { useAppStore } from "@/lib/store/useAppStore";
import { teamClient } from "@/lib/api/team-client";
import { getAppErrorMessage } from "@/lib/api/transport";

export function useTeams() {
  const queryClient = useQueryClient();
  const { t } = useI18n();
  const serviceStatus = useAppStore((state) => state.serviceStatus);
  const { canAccessManagementRpc } = useRuntimeCapabilities();
  const isServiceReady = canAccessManagementRpc && serviceStatus.connected;
  const isPageActive = useDesktopPageActive("/teams/");
  const areTeamQueriesEnabled = useDeferredDesktopActivation(
    isServiceReady && isPageActive,
  );

  const listQuery = useQuery({
    queryKey: ["teams"],
    queryFn: () => teamClient.list(),
    enabled: areTeamQueriesEnabled,
    retry: 1,
  });

  const invalidateTeams = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["teams"] }),
      queryClient.invalidateQueries({ queryKey: ["startup-snapshot"] }),
    ]);
  };

  const addFromAccountMutation = useMutation({
    mutationFn: (accountId: string) => teamClient.addFromAccount(accountId),
    onSuccess: async () => {
      await invalidateTeams();
      toast.success(t("已加入团队管理"));
    },
    onError: (error: unknown) => {
      toast.error(`${t("加入团队管理失败")}: ${getAppErrorMessage(error)}`);
    },
  });

  const syncTeamMutation = useMutation({
    mutationFn: (teamId: string) => teamClient.sync(teamId),
    onSuccess: async () => {
      await invalidateTeams();
      toast.success(t("团队信息已同步"));
    },
    onError: (error: unknown) => {
      toast.error(`${t("同步团队失败")}: ${getAppErrorMessage(error)}`);
    },
  });

  const inviteMutation = useMutation({
    mutationFn: ({ teamId, emails }: { teamId: string; emails: string[] }) =>
      teamClient.invite(teamId, emails),
    onSuccess: async (result) => {
      await invalidateTeams();
      toast.success(
        result.message || t("已发送 {count} 个邀请", { count: result.invitedCount }),
      );
    },
    onError: (error: unknown) => {
      toast.error(`${t("发送邀请失败")}: ${getAppErrorMessage(error)}`);
    },
  });

  const deleteMutation = useMutation({
    mutationFn: (teamId: string) => teamClient.delete(teamId),
    onSuccess: async () => {
      await invalidateTeams();
      toast.success(t("已移出团队管理"));
    },
    onError: (error: unknown) => {
      toast.error(`${t("移出团队管理失败")}: ${getAppErrorMessage(error)}`);
    },
  });

  return {
    teams: listQuery.data || [],
    isLoading: isServiceReady && (listQuery.isLoading || !areTeamQueriesEnabled),
    isServiceReady,
    addFromAccount: async (accountId: string) => {
      return await addFromAccountMutation.mutateAsync(accountId);
    },
    syncTeam: async (teamId: string) => {
      return await syncTeamMutation.mutateAsync(teamId);
    },
    inviteMembers: async (teamId: string, emails: string[]) => {
      return await inviteMutation.mutateAsync({ teamId, emails });
    },
    deleteTeam: async (teamId: string) => {
      await deleteMutation.mutateAsync(teamId);
    },
    refreshTeams: async () => {
      if (!isServiceReady) return;
      await invalidateTeams();
    },
    isAddingFromAccountId:
      addFromAccountMutation.isPending &&
      typeof addFromAccountMutation.variables === "string"
        ? addFromAccountMutation.variables
        : "",
    isSyncingTeamId:
      syncTeamMutation.isPending && typeof syncTeamMutation.variables === "string"
        ? syncTeamMutation.variables
        : "",
    isInvitingTeamId:
      inviteMutation.isPending &&
      inviteMutation.variables &&
      typeof inviteMutation.variables === "object"
        ? String(inviteMutation.variables.teamId || "")
        : "",
    isDeletingTeamId:
      deleteMutation.isPending && typeof deleteMutation.variables === "string"
        ? deleteMutation.variables
        : "",
  };
}
