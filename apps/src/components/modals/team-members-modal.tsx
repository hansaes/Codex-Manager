"use client";

import { useEffect, useMemo, useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Textarea } from "@/components/ui/textarea";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { ManagedTeam, ManagedTeamMember } from "@/types";
import { useI18n } from "@/lib/i18n/provider";
import { formatTsFromSeconds } from "@/lib/utils/usage";

interface TeamMembersModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  team: ManagedTeam | null;
  members: ManagedTeamMember[];
  isLoading: boolean;
  isInviting: boolean;
  onInvite: (emails: string[]) => Promise<void>;
}

export function TeamMembersModal({
  open,
  onOpenChange,
  team,
  members,
  isLoading,
  isInviting,
  onInvite,
}: TeamMembersModalProps) {
  const { t } = useI18n();
  const [emailsDraft, setEmailsDraft] = useState("");

  useEffect(() => {
    if (!open) {
      setEmailsDraft("");
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

  const handleInvite = async () => {
    const emails = emailsDraft
      .split(/[\n,]/)
      .map((item) => item.trim())
      .filter(Boolean);
    if (!emails.length) {
      return;
    }
    await onInvite(emails);
    setEmailsDraft("");
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="glass-card border-none sm:max-w-[920px]">
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

        <div className="grid gap-4">
          <div className="grid gap-2 rounded-xl bg-muted/20 p-4">
            <Label htmlFor="team-invite-emails">{t("邀请子号邮箱")}</Label>
            <Textarea
              id="team-invite-emails"
              value={emailsDraft}
              onChange={(event) => setEmailsDraft(event.target.value)}
              placeholder={t("支持多个邮箱，逗号或换行分隔")}
              disabled={!team || isInviting}
              className="min-h-[96px]"
            />
            <div className="flex items-center justify-between text-xs text-muted-foreground">
              <span>{t("邀请会直接发送到对应邮箱")}</span>
              <Button
                size="sm"
                onClick={() => void handleInvite()}
                disabled={!team || isInviting || !emailsDraft.trim()}
              >
                {isInviting ? t("发送中...") : t("发送邀请")}
              </Button>
            </div>
          </div>

          <div className="grid gap-4 lg:grid-cols-2">
            <div className="grid gap-2">
              <div className="flex items-center justify-between">
                <h3 className="text-sm font-semibold">{t("已加入成员")}</h3>
                <Badge variant="secondary">{joinedMembers.length}</Badge>
              </div>
              <div className="overflow-hidden rounded-xl border">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>{t("邮箱")}</TableHead>
                      <TableHead>{t("角色")}</TableHead>
                      <TableHead>{t("加入时间")}</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {isLoading ? (
                      <TableRow>
                        <TableCell colSpan={3} className="text-center text-muted-foreground">
                          {t("加载中...")}
                        </TableCell>
                      </TableRow>
                    ) : joinedMembers.length === 0 ? (
                      <TableRow>
                        <TableCell colSpan={3} className="text-center text-muted-foreground">
                          {t("暂无已加入成员")}
                        </TableCell>
                      </TableRow>
                    ) : (
                      joinedMembers.map((member) => (
                        <TableRow key={`${member.status}-${member.email}`}>
                          <TableCell className="font-mono text-xs">{member.email}</TableCell>
                          <TableCell>{member.role || "-"}</TableCell>
                          <TableCell>
                            {formatTsFromSeconds(member.addedAt, t("未知时间"))}
                          </TableCell>
                        </TableRow>
                      ))
                    )}
                  </TableBody>
                </Table>
              </div>
            </div>

            <div className="grid gap-2">
              <div className="flex items-center justify-between">
                <h3 className="text-sm font-semibold">{t("待接受邀请")}</h3>
                <Badge variant="secondary">{invitedMembers.length}</Badge>
              </div>
              <div className="overflow-hidden rounded-xl border">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>{t("邮箱")}</TableHead>
                      <TableHead>{t("角色")}</TableHead>
                      <TableHead>{t("邀请时间")}</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {isLoading ? (
                      <TableRow>
                        <TableCell colSpan={3} className="text-center text-muted-foreground">
                          {t("加载中...")}
                        </TableCell>
                      </TableRow>
                    ) : invitedMembers.length === 0 ? (
                      <TableRow>
                        <TableCell colSpan={3} className="text-center text-muted-foreground">
                          {t("暂无待接受邀请")}
                        </TableCell>
                      </TableRow>
                    ) : (
                      invitedMembers.map((member) => (
                        <TableRow key={`${member.status}-${member.email}`}>
                          <TableCell className="font-mono text-xs">{member.email}</TableCell>
                          <TableCell>{member.role || "-"}</TableCell>
                          <TableCell>
                            {formatTsFromSeconds(member.addedAt, t("未知时间"))}
                          </TableCell>
                        </TableRow>
                      ))
                    )}
                  </TableBody>
                </Table>
              </div>
            </div>
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t("关闭")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
