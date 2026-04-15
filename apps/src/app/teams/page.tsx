"use client";

import { useEffect, useMemo, useState } from "react";
import { RefreshCw, Trash2, Users } from "lucide-react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import { TeamMembersModal } from "@/components/modals/team-members-modal";
import { ConfirmDialog } from "@/components/modals/confirm-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { usePageTransitionReady } from "@/hooks/usePageTransitionReady";
import { useTeams } from "@/hooks/useTeams";
import { useI18n } from "@/lib/i18n/provider";
import { teamClient } from "@/lib/api/team-client";
import { getAppErrorMessage } from "@/lib/api/transport";
import { formatTsFromSeconds } from "@/lib/utils/usage";
import { ManagedTeamMembersResult } from "@/types";

export default function TeamsPage() {
  const { t } = useI18n();
  const {
    teams,
    isLoading,
    isServiceReady,
    syncTeam,
    inviteMembers,
    deleteTeam,
    isSyncingTeamId,
    isInvitingTeamId,
    isDeletingTeamId,
  } = useTeams();
  const [selectedTeamId, setSelectedTeamId] = useState("");
  const [deleteTeamId, setDeleteTeamId] = useState("");

  const selectedTeam = useMemo(
    () => teams.find((item) => item.id === selectedTeamId) || null,
    [selectedTeamId, teams],
  );

  const membersQuery = useQuery<ManagedTeamMembersResult>({
    queryKey: ["team-members", selectedTeamId],
    queryFn: () => teamClient.members(selectedTeamId),
    enabled: Boolean(selectedTeamId && isServiceReady),
    retry: 1,
  });

  const inviteMutation = useMutation({
    mutationFn: (emails: string[]) => {
      if (!selectedTeamId) {
        throw new Error("teamId is required");
      }
      return inviteMembers(selectedTeamId, emails);
    },
    onSuccess: async () => {
      await membersQuery.refetch();
    },
    onError: (error: unknown) => {
      toast.error(getAppErrorMessage(error));
    },
  });

  usePageTransitionReady("/teams/", !isServiceReady || !isLoading);

  useEffect(() => {
    if (!selectedTeamId) return;
    if (!teams.some((item) => item.id === selectedTeamId)) {
      setSelectedTeamId("");
    }
  }, [selectedTeamId, teams]);

  const renderStatusBadge = (status: string) => {
    const normalized = String(status || "").trim().toLowerCase();
    if (normalized === "active") {
      return (
        <Badge className="border-green-500/20 bg-green-500/10 text-green-600">
          {t("可用")}
        </Badge>
      );
    }
    if (normalized === "full") {
      return (
        <Badge className="border-amber-500/20 bg-amber-500/10 text-amber-600">
          {t("已满")}
        </Badge>
      );
    }
    if (normalized === "expired") {
      return (
        <Badge className="border-red-500/20 bg-red-500/10 text-red-600">
          {t("已过期")}
        </Badge>
      );
    }
    return <Badge variant="secondary">{status || t("未知")}</Badge>;
  };

  return (
    <div className="space-y-6 animate-in fade-in duration-500">
      {!isServiceReady ? (
        <Card className="glass-card border-none shadow-sm">
          <CardContent className="pt-6 text-sm text-muted-foreground">
            {t("服务未连接")}
          </CardContent>
        </Card>
      ) : null}

      <div>
        <p className="mt-1 text-sm text-muted-foreground">
          {t("从账号管理里把 Team 母号复制进来，这里负责同步席位、查看成员和发送邀请。")}
        </p>
      </div>

      <Card className="glass-card overflow-hidden border-none py-0 shadow-xl backdrop-blur-md">
        <CardContent className="p-0">
          <Table className="w-full table-fixed">
            <TableHeader>
              <TableRow>
                <TableHead>{t("账号信息")}</TableHead>
                <TableHead>{t("Team 名称")}</TableHead>
                <TableHead className="w-[120px]">{t("成员数")}</TableHead>
                <TableHead className="w-[140px]">{t("到期时间")}</TableHead>
                <TableHead className="w-[120px]">{t("状态")}</TableHead>
                <TableHead className="w-[200px] text-right">{t("操作")}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {isLoading ? (
                <TableRow>
                  <TableCell colSpan={6} className="h-32 text-center text-muted-foreground">
                    {t("加载中...")}
                  </TableCell>
                </TableRow>
              ) : teams.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={6} className="h-40 text-center text-muted-foreground">
                    {t("暂无团队母号，请先去账号管理添加")}
                  </TableCell>
                </TableRow>
              ) : (
                teams.map((team) => (
                  <TableRow
                    key={team.id}
                    className="cursor-pointer"
                    onDoubleClick={() => setSelectedTeamId(team.id)}
                  >
                    <TableCell>
                      <div className="grid gap-1">
                        <span className="font-medium">
                          {team.sourceAccountLabel || team.sourceAccountId}
                        </span>
                        <span className="font-mono text-[11px] text-muted-foreground">
                          {team.sourceAccountId}
                        </span>
                      </div>
                    </TableCell>
                    <TableCell>
                      <div className="grid gap-1">
                        <span>{team.teamName || "-"}</span>
                        {team.subscriptionPlan ? (
                          <span className="text-[11px] text-muted-foreground">
                            {team.subscriptionPlan}
                          </span>
                        ) : null}
                      </div>
                    </TableCell>
                    <TableCell>
                      <div className="grid gap-1">
                        <span className="font-medium">
                          {team.currentMembers}/{team.maxMembers}
                        </span>
                        {team.pendingInvites > 0 ? (
                          <span className="text-[11px] text-muted-foreground">
                            {t("待邀请")} {team.pendingInvites}
                          </span>
                        ) : null}
                      </div>
                    </TableCell>
                    <TableCell>
                      {formatTsFromSeconds(team.expiresAt, t("未知时间"))}
                    </TableCell>
                    <TableCell>{renderStatusBadge(team.status)}</TableCell>
                    <TableCell>
                      <div className="flex items-center justify-end gap-2">
                        <Button
                          variant="outline"
                          size="sm"
                          disabled={!isServiceReady || isSyncingTeamId === team.id}
                          onClick={() => void syncTeam(team.id)}
                        >
                          <RefreshCw
                            className={
                              isSyncingTeamId === team.id
                                ? "mr-1 h-4 w-4 animate-spin"
                                : "mr-1 h-4 w-4"
                            }
                          />
                          {t("同步")}
                        </Button>
                        <Button
                          variant="outline"
                          size="sm"
                          disabled={!isServiceReady}
                          onClick={() => setSelectedTeamId(team.id)}
                        >
                          <Users className="mr-1 h-4 w-4" />
                          {t("成员")}
                        </Button>
                        <Button
                          variant="outline"
                          size="sm"
                          className="text-red-500"
                          disabled={!isServiceReady || isDeletingTeamId === team.id}
                          onClick={() => setDeleteTeamId(team.id)}
                        >
                          <Trash2 className="mr-1 h-4 w-4" />
                          {t("移除")}
                        </Button>
                      </div>
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      <TeamMembersModal
        open={Boolean(selectedTeamId)}
        onOpenChange={(open) => {
          if (!open) {
            setSelectedTeamId("");
          }
        }}
        team={selectedTeam}
        members={membersQuery.data?.items || []}
        isLoading={membersQuery.isLoading}
        isInviting={inviteMutation.isPending && isInvitingTeamId === selectedTeamId}
        onInvite={async (emails) => {
          await inviteMutation.mutateAsync(emails);
        }}
      />

      <ConfirmDialog
        open={Boolean(deleteTeamId)}
        onOpenChange={(open) => {
          if (!open) {
            setDeleteTeamId("");
          }
        }}
        title={t("移出团队管理")}
        description={t("确认将这个母号从团队管理中移除吗？")}
        confirmText={t("移除")}
        confirmVariant="destructive"
        onConfirm={() => {
          if (!deleteTeamId) return;
          void deleteTeam(deleteTeamId);
          setDeleteTeamId("");
          if (selectedTeamId === deleteTeamId) {
            setSelectedTeamId("");
          }
        }}
      />
    </div>
  );
}
