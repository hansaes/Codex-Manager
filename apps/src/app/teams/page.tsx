"use client";

import { useDeferredValue, useEffect, useMemo, useState } from "react";
import { Building2, RefreshCw, Trash2, Users } from "lucide-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import {
  buildManagedTeamStats,
  filterManagedTeams,
  mergeManagedTeamInviteMembers,
  type ManagedTeamStatusFilter,
  removeManagedTeamMemberFromCache,
} from "@/app/teams/teams-page-helpers";
import { ConfirmDialog } from "@/components/modals/confirm-dialog";
import { TeamMembersModal } from "@/components/modals/team-members-modal";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
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
import { teamClient } from "@/lib/api/team-client";
import { getAppErrorMessage } from "@/lib/api/transport";
import { useI18n } from "@/lib/i18n/provider";
import { formatTsFromSeconds } from "@/lib/utils/usage";
import { ManagedTeamMembersResult } from "@/types";

function OverviewStatCard({
  label,
  value,
  hint,
}: {
  label: string;
  value: string;
  hint: string;
}) {
  return (
    <div className="rounded-2xl border border-border/60 bg-background/45 p-4 shadow-sm">
      <p className="text-xs uppercase tracking-[0.14em] text-muted-foreground/80">
        {label}
      </p>
      <p className="mt-2 text-2xl font-semibold tracking-tight text-foreground">
        {value}
      </p>
      <p className="mt-1 text-xs leading-5 text-muted-foreground">{hint}</p>
    </div>
  );
}

function renderStatusBadge(status: string, t: (key: string) => string) {
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
}

export default function TeamsPage() {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const {
    teams,
    isLoading,
    isServiceReady,
    refreshTeams,
    syncTeam,
    inviteMembers,
    removeMember,
    revokeInvite,
    deleteTeam,
    isRefreshingTeams,
    isSyncingTeamId,
    isInvitingTeamId,
    isRemovingMemberKey,
    isRevokingInviteKey,
    isDeletingTeamId,
  } = useTeams();
  const [search, setSearch] = useState("");
  const deferredSearch = useDeferredValue(search);
  const [statusFilter, setStatusFilter] =
    useState<ManagedTeamStatusFilter>("all");
  const [selectedTeamId, setSelectedTeamId] = useState("");
  const [deleteTeamId, setDeleteTeamId] = useState("");

  const statusOptions = useMemo(
    () => [
      { value: "all" as ManagedTeamStatusFilter, label: t("全部状态") },
      { value: "active" as ManagedTeamStatusFilter, label: t("可用") },
      { value: "full" as ManagedTeamStatusFilter, label: t("已满") },
      { value: "expired" as ManagedTeamStatusFilter, label: t("已过期") },
    ],
    [t],
  );

  const filteredTeams = useMemo(
    () =>
      filterManagedTeams(teams, {
        search: deferredSearch,
        status: statusFilter,
      }),
    [deferredSearch, statusFilter, teams],
  );
  const filteredStats = useMemo(
    () => buildManagedTeamStats(filteredTeams),
    [filteredTeams],
  );
  const totalStats = useMemo(() => buildManagedTeamStats(teams), [teams]);
  const hasFilters = deferredSearch.trim().length > 0 || statusFilter !== "all";
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
    onSuccess: (result) => {
      queryClient.setQueryData<ManagedTeamMembersResult>(
        ["team-members", selectedTeamId],
        (current) =>
          mergeManagedTeamInviteMembers(
            current,
            {
              teamId: result.teamId,
              invited: result.invited,
              pendingSync: result.pendingSync,
            },
            selectedTeamId,
            Math.floor(Date.now() / 1000),
          ),
      );
      void membersQuery.refetch();
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

  return (
    <div className="space-y-6 animate-in fade-in duration-500">
      {!isServiceReady ? (
        <Card className="glass-card border-none shadow-sm">
          <CardContent className="pt-6 text-sm text-muted-foreground">
            {t("服务未连接")}
          </CardContent>
        </Card>
      ) : null}

      <div className="space-y-1">
        <h1 className="text-3xl font-semibold tracking-tight">{t("团队管理")}</h1>
        <p className="max-w-4xl text-sm leading-6 text-muted-foreground">
          {t(
            "从账号管理同步 Team 母号后，可在这里搜索团队、查看席位和进入成员邀请与管理。",
          )}
        </p>
      </div>

      <Card className="glass-card border-none shadow-md backdrop-blur-md">
        <CardContent className="grid gap-3 pt-0 lg:grid-cols-[minmax(0,1fr)_168px_auto] lg:items-center">
          <div className="min-w-0">
            <Input
              placeholder={t("搜索团队名 / 母号 / Account ID...")}
              className="glass-card h-10 rounded-xl px-3"
              value={search}
              onChange={(event) => setSearch(event.target.value)}
            />
          </div>

          <Select
            value={statusFilter}
            onValueChange={(value) =>
              setStatusFilter(value as ManagedTeamStatusFilter)
            }
          >
            <SelectTrigger className="h-10 w-full rounded-xl bg-card/50">
              <SelectValue placeholder={t("全部状态")} />
            </SelectTrigger>
            <SelectContent>
              {statusOptions.map((option) => (
                <SelectItem key={option.value} value={option.value}>
                  {option.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>

          <div className="ml-auto flex items-center gap-2">
            <Button
              variant="outline"
              className="glass-card h-10 min-w-[106px] gap-2 rounded-xl px-3"
              disabled={!isServiceReady || isRefreshingTeams}
              onClick={() => void refreshTeams()}
            >
              <RefreshCw
                className={
                  isRefreshingTeams ? "h-4 w-4 animate-spin" : "h-4 w-4"
                }
              />
              <span className="text-sm font-medium">
                {isRefreshingTeams ? t("刷新中...") : t("刷新列表")}
              </span>
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card className="glass-card overflow-hidden border-none py-0 shadow-xl backdrop-blur-md">
        <CardHeader className="border-b border-border/50 pb-4">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div className="space-y-1">
              <CardTitle className="flex items-center gap-2 text-base">
                <Building2 className="h-4 w-4 text-primary" />
                {t("团队列表")}
              </CardTitle>
              <CardDescription>
                {t("双击表格行可直接打开成员视图；邀请结果会容忍上游同步延迟。")}
              </CardDescription>
            </div>
            <Badge variant="secondary" className="rounded-full px-3 py-1 text-xs">
              {t("显示 {visible} / {total} 个团队", {
                visible: filteredTeams.length,
                total: totalStats.totalTeams,
              })}
            </Badge>
          </div>
        </CardHeader>
        <CardContent className="space-y-4 p-4">
          <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
            <OverviewStatCard
              label={t("当前结果")}
              value={String(filteredStats.totalTeams)}
              hint={
                hasFilters
                  ? t("当前搜索和筛选命中的团队数量")
                  : t("已纳入团队管理的母号总数")
              }
            />
            <OverviewStatCard
              label={t("活跃团队")}
              value={String(filteredStats.activeTeams)}
              hint={t("状态为可用的团队数量")}
            />
            <OverviewStatCard
              label={t("已占席位")}
              value={String(filteredStats.occupiedSlots)}
              hint={t("当前结果中的席位占用总和")}
            />
            <OverviewStatCard
              label={t("待接受邀请")}
              value={String(filteredStats.pendingInvites)}
              hint={t("当前结果中的待接受邀请数量")}
            />
          </div>

          <div className="overflow-hidden rounded-2xl border border-border/50">
            <div className="overflow-x-auto">
              <Table className="min-w-[900px] table-fixed">
                <TableHeader>
                  <TableRow>
                    <TableHead className="w-[220px]">{t("账号信息")}</TableHead>
                    <TableHead>{t("Team 名称")}</TableHead>
                    <TableHead className="w-[120px]">{t("成员数")}</TableHead>
                    <TableHead className="w-[140px]">{t("到期时间")}</TableHead>
                    <TableHead className="w-[120px]">{t("状态")}</TableHead>
                    <TableHead className="w-[220px] text-right">
                      {t("操作")}
                    </TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {isLoading ? (
                    <TableRow>
                      <TableCell
                        colSpan={6}
                        className="h-32 text-center text-muted-foreground"
                      >
                        {t("加载中...")}
                      </TableCell>
                    </TableRow>
                  ) : teams.length === 0 ? (
                    <TableRow>
                      <TableCell
                        colSpan={6}
                        className="h-40 text-center text-muted-foreground"
                      >
                        {t("暂无团队母号，请先去账号管理添加")}
                      </TableCell>
                    </TableRow>
                  ) : filteredTeams.length === 0 ? (
                    <TableRow>
                      <TableCell
                        colSpan={6}
                        className="h-40 text-center text-muted-foreground"
                      >
                        {t("当前搜索或状态筛选下没有匹配的团队")}
                      </TableCell>
                    </TableRow>
                  ) : (
                    filteredTeams.map((team) => (
                      <TableRow
                        key={team.id}
                        className="cursor-pointer transition-colors hover:bg-muted/20"
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
                        <TableCell>{renderStatusBadge(team.status, t)}</TableCell>
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
            </div>
          </div>
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
        removingMemberKey={isRemovingMemberKey}
        revokingInviteKey={isRevokingInviteKey}
        onInvite={async (emails) => {
          await inviteMutation.mutateAsync(emails);
        }}
        onRemoveMember={async (member) => {
          if (!selectedTeamId || !member.userId) {
            toast.error(t("缺少可用的成员标识"));
            return;
          }
          await removeMember(selectedTeamId, member.userId);
          queryClient.setQueryData<ManagedTeamMembersResult>(
            ["team-members", selectedTeamId],
            (current) => removeManagedTeamMemberFromCache(current, member),
          );
          void membersQuery.refetch();
        }}
        onRevokeInvite={async (member) => {
          if (!selectedTeamId || !member.email) {
            toast.error(t("缺少可用的邀请邮箱"));
            return;
          }
          await revokeInvite(selectedTeamId, member.email);
          queryClient.setQueryData<ManagedTeamMembersResult>(
            ["team-members", selectedTeamId],
            (current) => removeManagedTeamMemberFromCache(current, member),
          );
          void membersQuery.refetch();
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
