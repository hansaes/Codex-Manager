"use client";

import { type ReactNode, useEffect, useMemo, useState } from "react";
import {
  Clock3,
  MailPlus,
  RotateCcw,
  UserMinus,
  UsersRound,
} from "lucide-react";
import { ConfirmDialog } from "@/components/modals/confirm-dialog";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { useI18n } from "@/lib/i18n/provider";
import { formatTsFromSeconds } from "@/lib/utils/usage";
import { ManagedTeam, ManagedTeamMember } from "@/types";

interface TeamMembersModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  team: ManagedTeam | null;
  members: ManagedTeamMember[];
  isLoading: boolean;
  isInviting: boolean;
  removingMemberKey: string;
  revokingInviteKey: string;
  onInvite: (emails: string[]) => Promise<void>;
  onRemoveMember: (member: ManagedTeamMember) => Promise<void>;
  onRevokeInvite: (member: ManagedTeamMember) => Promise<void>;
}

type PendingAction =
  | {
      type: "remove" | "revoke";
      member: ManagedTeamMember;
    }
  | null;

function MemberListCard({
  title,
  count,
  icon,
  items,
  isLoading,
  emptyText,
  actionLabel,
  pendingLabel,
  isDanger = false,
  actionIcon,
  getActionKey,
  activeActionKey,
  onAction,
}: {
  title: string;
  count: number;
  icon: ReactNode;
  items: ManagedTeamMember[];
  isLoading: boolean;
  emptyText: string;
  actionLabel: string;
  pendingLabel: string;
  isDanger?: boolean;
  actionIcon: ReactNode;
  getActionKey: (member: ManagedTeamMember) => string;
  activeActionKey: string;
  onAction: (member: ManagedTeamMember) => void;
}) {
  const { t } = useI18n();

  return (
    <Card className="border-border/60 bg-background/30 shadow-sm">
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between gap-3">
          <div className="flex items-center gap-2">
            {icon}
            <CardTitle className="text-sm font-semibold">{title}</CardTitle>
          </div>
          <Badge variant="secondary" className="rounded-full px-2.5 py-1">
            {count}
          </Badge>
        </div>
      </CardHeader>

      <CardContent>
        {isLoading ? (
          <div className="rounded-2xl border border-dashed border-border/70 bg-background/50 px-4 py-8 text-center text-sm text-muted-foreground">
            {t("加载中...")}
          </div>
        ) : items.length === 0 ? (
          <div className="rounded-2xl border border-dashed border-border/70 bg-background/50 px-4 py-8 text-center text-sm text-muted-foreground">
            {emptyText}
          </div>
        ) : (
          <div className="max-h-[250px] space-y-3 overflow-y-auto pr-1">
            {items.map((member) => {
              const actionKey = getActionKey(member);
              const isPending = activeActionKey === actionKey;

              return (
                <div
                  key={`${member.status}-${member.email}`}
                  className="flex flex-col gap-3 rounded-2xl border border-border/60 bg-background/55 p-4 md:flex-row md:items-center md:justify-between"
                >
                  <div className="min-w-0 flex-1 space-y-2">
                    <div className="break-all font-mono text-sm font-medium leading-5 text-foreground">
                      {member.email}
                    </div>
                    <div className="flex flex-wrap gap-2 text-xs text-muted-foreground">
                      <Badge variant="outline" className="rounded-full px-2.5 py-1">
                        {member.role || "-"}
                      </Badge>
                      <Badge variant="outline" className="rounded-full px-2.5 py-1">
                        {formatTsFromSeconds(member.addedAt, t("未知时间"))}
                      </Badge>
                      {member.status === "joined" ? (
                        <Badge
                          variant="secondary"
                          className="rounded-full bg-green-500/10 px-2.5 py-1 text-green-600"
                        >
                          {t("已加入")}
                        </Badge>
                      ) : (
                        <Badge
                          variant="secondary"
                          className="rounded-full bg-amber-500/10 px-2.5 py-1 text-amber-600"
                        >
                          {t("待接受")}
                        </Badge>
                      )}
                    </div>
                  </div>

                  <div className="flex shrink-0 items-center justify-end">
                    <Button
                      size="sm"
                      variant="outline"
                      className={isDanger ? "text-red-500" : "text-amber-600"}
                      disabled={isPending}
                      onClick={() => onAction(member)}
                    >
                      {!isPending ? actionIcon : null}
                      {isPending ? pendingLabel : actionLabel}
                    </Button>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

export function TeamMembersModal({
  open,
  onOpenChange,
  team,
  members,
  isLoading,
  isInviting,
  removingMemberKey,
  revokingInviteKey,
  onInvite,
  onRemoveMember,
  onRevokeInvite,
}: TeamMembersModalProps) {
  const { t } = useI18n();
  const [emailsDraft, setEmailsDraft] = useState("");
  const [pendingAction, setPendingAction] = useState<PendingAction>(null);

  useEffect(() => {
    if (!open) {
      setEmailsDraft("");
      setPendingAction(null);
    }
  }, [open]);

  const joinedMembers = useMemo(
    () => members.filter((item) => item.status === "joined"),
    [members],
  );
  const invitedMembers = useMemo(
    () => members.filter((item) => item.status !== "joined"),
    [members],
  );

  const parsedDraftEmails = useMemo(
    () =>
      emailsDraft
        .split(/[\n,]/)
        .map((item) => item.trim().toLowerCase())
        .filter((item) => item.includes("@")),
    [emailsDraft],
  );
  const uniqueDraftEmails = useMemo(
    () => Array.from(new Set(parsedDraftEmails)),
    [parsedDraftEmails],
  );
  const duplicateDraftCount = Math.max(
    parsedDraftEmails.length - uniqueDraftEmails.length,
    0,
  );
  const joinedEmailSet = useMemo(
    () => new Set(joinedMembers.map((item) => item.email.toLowerCase())),
    [joinedMembers],
  );
  const invitedEmailSet = useMemo(
    () => new Set(invitedMembers.map((item) => item.email.toLowerCase())),
    [invitedMembers],
  );
  const draftAlreadyJoined = useMemo(
    () => uniqueDraftEmails.filter((email) => joinedEmailSet.has(email)),
    [joinedEmailSet, uniqueDraftEmails],
  );
  const draftAlreadyInvited = useMemo(
    () => uniqueDraftEmails.filter((email) => invitedEmailSet.has(email)),
    [invitedEmailSet, uniqueDraftEmails],
  );
  const draftReadyToInvite = useMemo(
    () =>
      uniqueDraftEmails.filter(
        (email) => !joinedEmailSet.has(email) && !invitedEmailSet.has(email),
      ),
    [invitedEmailSet, joinedEmailSet, uniqueDraftEmails],
  );

  const handleInvite = async () => {
    if (!draftReadyToInvite.length) {
      return;
    }
    await onInvite(draftReadyToInvite);
    setEmailsDraft("");
  };

  return (
    <>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent className="glass-card border-none sm:max-w-[1040px]">
          <DialogHeader>
            <DialogTitle>
              {team?.teamName || team?.sourceAccountLabel || t("团队成员")}
            </DialogTitle>
            <DialogDescription>
              {team
                ? `${t("母号")}: ${team.sourceAccountLabel || team.sourceAccountId} · ${t("成员数")}: ${team.currentMembers}/${team.maxMembers}`
                : t("查看团队成员与待接受邀请")}
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4">
            <Card className="border-border/60 bg-background/35 shadow-sm">
              <CardHeader className="pb-2">
                <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
                  <div className="space-y-1">
                    <CardTitle className="flex items-center gap-2 text-sm">
                      <MailPlus className="h-4 w-4 text-primary" />
                      {t("邀请子号邮箱")}
                    </CardTitle>
                    <p className="text-xs text-muted-foreground">
                      {t("支持多个邮箱，逗号或换行分隔")}
                    </p>
                  </div>
                  <Button
                    size="sm"
                    className="min-w-[112px] self-start md:self-center"
                    onClick={() => void handleInvite()}
                    disabled={!team || isInviting || draftReadyToInvite.length === 0}
                  >
                    {isInviting ? t("发送中...") : t("发送邀请")}
                  </Button>
                </div>
              </CardHeader>

              <CardContent className="space-y-3">
                <div className="grid gap-2">
                  <Label htmlFor="team-invite-emails">{t("邮箱列表")}</Label>
                  <Textarea
                    id="team-invite-emails"
                    value={emailsDraft}
                    onChange={(event) => setEmailsDraft(event.target.value)}
                    placeholder={t("支持多个邮箱，逗号或换行分隔")}
                    disabled={!team || isInviting}
                    className="min-h-[76px] resize-none"
                  />
                </div>

                <div className="flex flex-wrap gap-2 text-xs">
                  <Badge variant="secondary" className="rounded-full px-3 py-1">
                    {t("可发送")} {draftReadyToInvite.length}
                  </Badge>
                  <Badge variant="secondary" className="rounded-full px-3 py-1">
                    {t("已加入")} {draftAlreadyJoined.length}
                  </Badge>
                  <Badge variant="secondary" className="rounded-full px-3 py-1">
                    {t("已邀请")} {draftAlreadyInvited.length}
                  </Badge>
                  <Badge variant="secondary" className="rounded-full px-3 py-1">
                    {t("重复")} {duplicateDraftCount}
                  </Badge>
                </div>

                {draftAlreadyJoined.length > 0 ? (
                  <p className="text-xs text-muted-foreground">
                    {t("以下邮箱已在团队中")}：{draftAlreadyJoined.join(", ")}
                  </p>
                ) : null}

                {draftAlreadyInvited.length > 0 ? (
                  <p className="text-xs text-muted-foreground">
                    {t("以下邮箱已存在待接受邀请")}：{draftAlreadyInvited.join(", ")}
                  </p>
                ) : null}

                <p className="text-xs text-muted-foreground">
                  {t("邀请会直接发送到对应邮箱，若列表未立即刷新会标记为待同步。")}
                </p>
              </CardContent>
            </Card>

            <MemberListCard
              title={t("已加入成员")}
              count={joinedMembers.length}
              icon={<UsersRound className="h-4 w-4 text-primary" />}
              items={joinedMembers}
              isLoading={isLoading}
              emptyText={t("暂无已加入成员")}
              actionLabel={t("移出")}
              pendingLabel={t("移出中...")}
              isDanger
              actionIcon={<UserMinus className="mr-1 h-4 w-4" />}
              activeActionKey={removingMemberKey}
              getActionKey={(member) => `${team?.id || ""}:${member.userId || ""}`}
              onAction={(member) => setPendingAction({ type: "remove", member })}
            />

            <MemberListCard
              title={t("待接受邀请")}
              count={invitedMembers.length}
              icon={<Clock3 className="h-4 w-4 text-primary" />}
              items={invitedMembers}
              isLoading={isLoading}
              emptyText={t("暂无待接受邀请")}
              actionLabel={t("撤回")}
              pendingLabel={t("撤回中...")}
              actionIcon={<RotateCcw className="mr-1 h-4 w-4" />}
              activeActionKey={revokingInviteKey}
              getActionKey={(member) => `${team?.id || ""}:${member.email}`}
              onAction={(member) => setPendingAction({ type: "revoke", member })}
            />
          </div>

          <DialogFooter>
            <Button variant="outline" onClick={() => onOpenChange(false)}>
              {t("关闭")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <ConfirmDialog
        open={pendingAction?.type === "remove"}
        onOpenChange={(nextOpen) => {
          if (!nextOpen) {
            setPendingAction(null);
          }
        }}
        title={t("移出成员")}
        description={
          pendingAction?.member
            ? `${t("确认将成员移出团队吗？")} ${pendingAction.member.email}`
            : t("确认将成员移出团队吗？")
        }
        confirmText={t("移出")}
        confirmVariant="destructive"
        onConfirm={() => {
          if (!pendingAction || pendingAction.type !== "remove") return;
          void onRemoveMember(pendingAction.member);
          setPendingAction(null);
        }}
      />

      <ConfirmDialog
        open={pendingAction?.type === "revoke"}
        onOpenChange={(nextOpen) => {
          if (!nextOpen) {
            setPendingAction(null);
          }
        }}
        title={t("撤回邀请")}
        description={
          pendingAction?.member
            ? `${t("确认撤回这个邀请吗？")} ${pendingAction.member.email}`
            : t("确认撤回这个邀请吗？")
        }
        confirmText={t("撤回")}
        confirmVariant="destructive"
        onConfirm={() => {
          if (!pendingAction || pendingAction.type !== "revoke") return;
          void onRevokeInvite(pendingAction.member);
          setPendingAction(null);
        }}
      />
    </>
  );
}
