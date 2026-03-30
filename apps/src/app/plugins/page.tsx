"use client";

import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Download,
  Play,
  RefreshCw,
  Rocket,
  Trash2,
  ToggleLeft,
  ToggleRight,
} from "lucide-react";
import { toast } from "sonner";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { useDesktopPageActive } from "@/hooks/useDesktopPageActive";
import { useDeferredDesktopActivation } from "@/hooks/useDeferredDesktopActivation";
import { usePageTransitionReady } from "@/hooks/usePageTransitionReady";
import { appClient } from "@/lib/api/app-client";
import { pluginClient } from "@/lib/api/plugin-client";
import { useAppStore } from "@/lib/store/useAppStore";
import { cn } from "@/lib/utils";
import {
  InstalledPluginSummary,
  PluginCatalogEntry,
  PluginRunLogSummary,
  PluginTaskSummary,
} from "@/types";

function PermissionBadge({ permission }: { permission: string }) {
  return (
    <Badge variant="secondary" className="mr-1.5 mb-1">
      {permission}
    </Badge>
  );
}

function StatusBadge({ status }: { status: string }) {
  const normalized = status.toLowerCase();
  const toneClass =
    normalized === "enabled"
      ? "border-emerald-500/20 bg-emerald-500/10 text-emerald-600"
      : normalized === "broken"
        ? "border-red-500/20 bg-red-500/10 text-red-600"
        : "border-amber-500/20 bg-amber-500/10 text-amber-600";
  return <Badge className={toneClass}>{status}</Badge>;
}

function PluginCard({
  item,
  onInstall,
}: {
  item: PluginCatalogEntry;
  onInstall: (entry: PluginCatalogEntry) => void;
}) {
  return (
    <Card className="glass-card border-none shadow-sm">
      <CardHeader className="space-y-2">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <CardTitle className="text-base">{item.name}</CardTitle>
            <CardDescription className="mt-1 line-clamp-2">
              {item.description || "暂无描述"}
            </CardDescription>
          </div>
          <Badge variant="secondary">{item.version}</Badge>
        </div>
        <div className="flex flex-wrap gap-2 text-xs text-muted-foreground">
          {item.author ? <span>作者：{item.author}</span> : null}
          {item.sourceUrl ? <span>来源：{item.sourceUrl}</span> : null}
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        <div>
          <p className="mb-1 text-xs text-muted-foreground">权限</p>
          <div>
            {item.permissions.length > 0 ? (
              item.permissions.map((permission) => (
                <PermissionBadge key={permission} permission={permission} />
              ))
            ) : (
              <span className="text-sm text-muted-foreground">无需额外权限</span>
            )}
          </div>
        </div>
        <div>
          <p className="mb-1 text-xs text-muted-foreground">任务</p>
          <div className="space-y-2">
            {item.tasks.map((task) => (
              <div
                key={task.id}
                className="rounded-xl border border-border/60 bg-background/60 p-3 text-sm"
              >
                <div className="flex items-center justify-between gap-2">
                  <div className="min-w-0">
                    <div className="font-medium">{task.name}</div>
                    <div className="text-xs text-muted-foreground">
                      {task.scheduleKind === "manual"
                        ? "手动"
                        : `每 ${task.intervalSeconds || 0} 秒`}
                      {" · "}
                      {task.entrypoint}
                    </div>
                  </div>
                  <Badge variant="outline">{task.enabled ? "启用" : "禁用"}</Badge>
                </div>
                {task.description ? (
                  <div className="mt-1 text-xs text-muted-foreground">{task.description}</div>
                ) : null}
              </div>
            ))}
          </div>
        </div>
        <div className="flex justify-end">
          <Button onClick={() => onInstall(item)} className="gap-2">
            <Download className="h-4 w-4" />
            安装
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}

function InstalledPluginCard({
  item,
  tasks,
  logs,
  onEnable,
  onDisable,
  onUpdate,
  onUninstall,
  onRunTask,
}: {
  item: InstalledPluginSummary;
  tasks: PluginTaskSummary[];
  logs: PluginRunLogSummary[];
  onEnable: (pluginId: string) => void;
  onDisable: (pluginId: string) => void;
  onUpdate: (pluginId: string, sourceUrl?: string | null) => void;
  onUninstall: (pluginId: string) => void;
  onRunTask: (taskId: string) => void;
}) {
  return (
    <Card className="glass-card border-none shadow-sm">
      <CardHeader className="space-y-2">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <CardTitle className="text-base">{item.name}</CardTitle>
            <CardDescription className="mt-1 line-clamp-2">
              {item.description || "暂无描述"}
            </CardDescription>
          </div>
          <div className="flex items-center gap-2">
            <Badge variant="secondary">{item.version}</Badge>
            <StatusBadge status={item.status} />
          </div>
        </div>
        <div className="flex flex-wrap gap-2 text-xs text-muted-foreground">
          {item.author ? <span>作者：{item.author}</span> : null}
          {item.sourceUrl ? <span>来源：{item.sourceUrl}</span> : null}
          <span>任务 {item.enabledTaskCount}/{item.taskCount}</span>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        <div>
          <p className="mb-1 text-xs text-muted-foreground">已授予权限</p>
          <div>
            {item.permissions.length > 0 ? (
              item.permissions.map((permission) => (
                <PermissionBadge key={permission} permission={permission} />
              ))
            ) : (
              <span className="text-sm text-muted-foreground">无</span>
            )}
          </div>
        </div>
        <div>
          <p className="mb-2 text-xs text-muted-foreground">任务</p>
          <div className="space-y-2">
            {tasks.length > 0 ? (
              tasks.map((task) => (
                <div
                  key={task.id}
                  className="rounded-xl border border-border/60 bg-background/60 p-3 text-sm"
                >
                  <div className="flex items-center justify-between gap-2">
                    <div className="min-w-0">
                      <div className="font-medium">{task.name}</div>
                      <div className="text-xs text-muted-foreground">
                        {task.scheduleKind === "manual"
                          ? "手动"
                          : `每 ${task.intervalSeconds || 0} 秒`}
                        {" · "}
                        {task.entrypoint}
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      <Badge variant="outline">{task.enabled ? "启用" : "停用"}</Badge>
                      <Button size="sm" variant="secondary" onClick={() => onRunTask(task.id)}>
                        <Play className="mr-1.5 h-3.5 w-3.5" />
                        运行
                      </Button>
                    </div>
                  </div>
                  {task.lastError ? (
                    <div className="mt-1 text-xs text-red-500">{task.lastError}</div>
                  ) : null}
                </div>
              ))
            ) : (
              <div className="text-sm text-muted-foreground">暂无任务</div>
            )}
          </div>
        </div>
        <div>
          <p className="mb-2 text-xs text-muted-foreground">最近运行</p>
          <div className="space-y-2">
            {logs.length > 0 ? (
              logs.slice(0, 3).map((log) => (
                <div
                  key={log.id}
                  className="rounded-xl border border-border/60 bg-background/60 p-3 text-xs"
                >
                  <div className="flex items-center justify-between gap-2">
                    <div className="font-medium">
                      {log.taskName || log.taskId || "未知任务"}
                    </div>
                    <Badge variant={log.status === "ok" ? "secondary" : "destructive"}>
                      {log.status}
                    </Badge>
                  </div>
                  <div className="mt-1 text-muted-foreground">
                    {log.error || (log.output ? JSON.stringify(log.output) : "无输出")}
                  </div>
                </div>
              ))
            ) : (
              <div className="text-sm text-muted-foreground">暂无日志</div>
            )}
          </div>
        </div>
        <div className="flex flex-wrap items-center justify-end gap-2">
          <Button
            variant="outline"
            className="gap-2"
            onClick={() => onUpdate(item.pluginId, item.sourceUrl)}
          >
            <RefreshCw className="h-4 w-4" />
            更新
          </Button>
          {item.status === "enabled" ? (
            <Button variant="outline" className="gap-2" onClick={() => onDisable(item.pluginId)}>
              <ToggleLeft className="h-4 w-4" />
              停用
            </Button>
          ) : (
            <Button variant="outline" className="gap-2" onClick={() => onEnable(item.pluginId)}>
              <ToggleRight className="h-4 w-4" />
              启用
            </Button>
          )}
          <Button variant="destructive" className="gap-2" onClick={() => onUninstall(item.pluginId)}>
            <Trash2 className="h-4 w-4" />
            卸载
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}

export default function PluginsPage() {
  const serviceReady = useAppStore((state) => state.serviceStatus.connected);
  const isPageActive = useDesktopPageActive("/plugins/");
  const isActivationReady = useDeferredDesktopActivation(serviceReady);
  usePageTransitionReady("/plugins/", !serviceReady);
  const queryClient = useQueryClient();
  const [sourceUrl, setSourceUrl] = useState("");

  const settingsQuery = useQuery({
    queryKey: ["plugin-settings"],
    queryFn: () => appClient.getSettings(),
    enabled: isPageActive && isActivationReady,
  });

  useEffect(() => {
    if (settingsQuery.data) {
      setSourceUrl(settingsQuery.data.pluginMarketSourceUrl || "");
    }
  }, [settingsQuery.data]);

  const catalogQuery = useQuery({
    queryKey: ["plugin-catalog", sourceUrl],
    queryFn: () => pluginClient.getCatalog(sourceUrl || undefined),
    enabled: isPageActive && isActivationReady,
  });

  const installedQuery = useQuery({
    queryKey: ["plugin-installed"],
    queryFn: () => pluginClient.listInstalled(),
    enabled: isPageActive && isActivationReady,
  });

  const tasksQuery = useQuery({
    queryKey: ["plugin-tasks"],
    queryFn: () => pluginClient.listTasks(),
    enabled: isPageActive && isActivationReady,
  });

  const logsQuery = useQuery({
    queryKey: ["plugin-logs"],
    queryFn: () => pluginClient.listLogs({ limit: 20 }),
    enabled: isPageActive && isActivationReady,
  });

  const saveSourceMutation = useMutation({
    mutationFn: async () => appClient.setSettings({ pluginMarketSourceUrl: sourceUrl }),
    onSuccess: (settings) => {
      setSourceUrl(settings.pluginMarketSourceUrl || "");
      toast.success("市场源已保存");
      void queryClient.invalidateQueries({ queryKey: ["plugin-catalog"] });
    },
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : "保存市场源失败");
    },
  });

  const installMutation = useMutation({
    mutationFn: (entry: PluginCatalogEntry) => pluginClient.install(entry),
    onSuccess: () => {
      toast.success("插件已安装");
      void queryClient.invalidateQueries({ queryKey: ["plugin-installed"] });
      void queryClient.invalidateQueries({ queryKey: ["plugin-tasks"] });
      void queryClient.invalidateQueries({ queryKey: ["plugin-logs"] });
    },
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : "安装失败");
    },
  });

  const toggleMutation = useMutation({
    mutationFn: async (payload: { pluginId: string; enabled: boolean }) =>
      payload.enabled ? pluginClient.enable(payload.pluginId) : pluginClient.disable(payload.pluginId),
    onSuccess: () => {
      toast.success("插件状态已更新");
      void queryClient.invalidateQueries({ queryKey: ["plugin-installed"] });
    },
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : "更新失败");
    },
  });

  const updateMutation = useMutation({
    mutationFn: (payload: { pluginId: string; sourceUrl?: string | null }) =>
      pluginClient.update(payload.pluginId, payload.sourceUrl || undefined),
    onSuccess: () => {
      toast.success("插件已更新");
      void queryClient.invalidateQueries({ queryKey: ["plugin-installed"] });
      void queryClient.invalidateQueries({ queryKey: ["plugin-tasks"] });
      void queryClient.invalidateQueries({ queryKey: ["plugin-logs"] });
    },
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : "更新失败");
    },
  });

  const uninstallMutation = useMutation({
    mutationFn: (pluginId: string) => pluginClient.uninstall(pluginId),
    onSuccess: () => {
      toast.success("插件已卸载");
      void queryClient.invalidateQueries({ queryKey: ["plugin-installed"] });
      void queryClient.invalidateQueries({ queryKey: ["plugin-tasks"] });
      void queryClient.invalidateQueries({ queryKey: ["plugin-logs"] });
    },
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : "卸载失败");
    },
  });

  const runTaskMutation = useMutation({
    mutationFn: (taskId: string) => pluginClient.runTask(taskId),
    onSuccess: () => {
      toast.success("任务已执行");
      void queryClient.invalidateQueries({ queryKey: ["plugin-installed"] });
      void queryClient.invalidateQueries({ queryKey: ["plugin-tasks"] });
      void queryClient.invalidateQueries({ queryKey: ["plugin-logs"] });
    },
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : "运行失败");
    },
  });

  const tasksByPluginId = useMemo(() => {
    const map = new Map<string, PluginTaskSummary[]>();
    for (const task of tasksQuery.data || []) {
      const items = map.get(task.pluginId) || [];
      items.push(task);
      map.set(task.pluginId, items);
    }
    return map;
  }, [tasksQuery.data]);

  const logsByPluginId = useMemo(() => {
    const map = new Map<string, PluginRunLogSummary[]>();
    for (const log of logsQuery.data || []) {
      const items = map.get(log.pluginId) || [];
      items.push(log);
      map.set(log.pluginId, items);
    }
    return map;
  }, [logsQuery.data]);

  const catalogItems = catalogQuery.data?.items || [];
  const installedItems = installedQuery.data || [];

  return (
    <div className="p-6 space-y-6">
      <div className="flex flex-col gap-2">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-2xl bg-primary/10 text-primary">
            <Rocket className="h-5 w-5" />
          </div>
          <div>
            <h1 className="text-2xl font-semibold">插件市场</h1>
            <p className="text-sm text-muted-foreground">接入第三方脚本、任务和自动化逻辑。</p>
          </div>
        </div>
      </div>

      <Card className="glass-card border-none shadow-sm">
        <CardHeader>
          <CardTitle>市场源</CardTitle>
          <CardDescription>留空时使用内置示例市场。填写远程 JSON 地址后可接入自己的插件仓库。</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-3 md:flex-row md:items-center">
          <Input
            value={sourceUrl}
            onChange={(event) => setSourceUrl(event.target.value)}
            placeholder="https://example.com/plugin-market.json"
            className="md:flex-1"
          />
          <div className="flex gap-2">
            <Button onClick={() => saveSourceMutation.mutate()} disabled={saveSourceMutation.isPending}>
              保存
            </Button>
            <Button
              variant="outline"
              onClick={() => void queryClient.invalidateQueries({ queryKey: ["plugin-catalog"] })}
            >
              <RefreshCw className="mr-2 h-4 w-4" />
              刷新
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card className="glass-card border-none shadow-sm">
        <CardHeader>
          <CardTitle>插件市场</CardTitle>
          <CardDescription>点击安装即可把插件拉入本机，安装后可再启用或运行任务。</CardDescription>
        </CardHeader>
        <CardContent>
          {catalogQuery.isLoading ? (
            <div className="grid gap-4 md:grid-cols-2">
              {Array.from({ length: 2 }).map((_, index) => (
                <Skeleton key={index} className="h-64 rounded-2xl" />
              ))}
            </div>
          ) : catalogItems.length > 0 ? (
            <div className="grid gap-4 md:grid-cols-2">
              {catalogItems.map((item) => (
                <PluginCard key={item.id} item={item} onInstall={(entry) => installMutation.mutate(entry)} />
              ))}
            </div>
          ) : (
            <div className="rounded-2xl border border-dashed border-border/60 p-10 text-center text-sm text-muted-foreground">
              暂无可安装插件
            </div>
          )}
        </CardContent>
      </Card>

      <Card className="glass-card border-none shadow-sm">
        <CardHeader>
          <CardTitle>已安装插件</CardTitle>
          <CardDescription>这里可以启用、停用、更新和卸载插件，也能直接运行任务。</CardDescription>
        </CardHeader>
        <CardContent>
          {installedQuery.isLoading ? (
            <div className="grid gap-4 md:grid-cols-2">
              {Array.from({ length: 2 }).map((_, index) => (
                <Skeleton key={index} className="h-72 rounded-2xl" />
              ))}
            </div>
          ) : installedItems.length > 0 ? (
            <div className="grid gap-4 md:grid-cols-2">
              {installedItems.map((item) => (
                <InstalledPluginCard
                  key={item.pluginId}
                  item={item}
                  tasks={tasksByPluginId.get(item.pluginId) || []}
                  logs={logsByPluginId.get(item.pluginId) || []}
                  onEnable={(pluginId) => toggleMutation.mutate({ pluginId, enabled: true })}
                  onDisable={(pluginId) => toggleMutation.mutate({ pluginId, enabled: false })}
                  onUpdate={(pluginId, sourceUrlValue) =>
                    updateMutation.mutate({ pluginId, sourceUrl: sourceUrlValue })
                  }
                  onUninstall={(pluginId) => {
                    if (window.confirm("确认卸载这个插件吗？")) {
                      uninstallMutation.mutate(pluginId);
                    }
                  }}
                  onRunTask={(taskId) => runTaskMutation.mutate(taskId)}
                />
              ))}
            </div>
          ) : (
            <div className="rounded-2xl border border-dashed border-border/60 p-10 text-center text-sm text-muted-foreground">
              还没有安装任何插件
            </div>
          )}
        </CardContent>
      </Card>

      <Card className="glass-card border-none shadow-sm">
        <CardHeader>
          <CardTitle>最近日志</CardTitle>
          <CardDescription>方便快速查看插件运行结果和错误。</CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          {logsQuery.isLoading ? (
            <Skeleton className="h-40 rounded-2xl" />
          ) : (logsQuery.data || []).length > 0 ? (
            (logsQuery.data || []).map((log) => (
              <div
                key={log.id}
                className={cn(
                  "rounded-2xl border p-3 text-sm",
                  log.status === "ok"
                    ? "border-emerald-500/20 bg-emerald-500/5"
                    : "border-red-500/20 bg-red-500/5",
                )}
              >
                <div className="flex items-center justify-between gap-2">
                  <div className="font-medium">
                    {log.pluginName || log.pluginId} · {log.taskName || log.taskId || "任务"}
                  </div>
                  <Badge variant={log.status === "ok" ? "secondary" : "destructive"}>
                    {log.status}
                  </Badge>
                </div>
                <div className="mt-1 text-xs text-muted-foreground">
                  {log.error || (log.output ? JSON.stringify(log.output) : "无输出")}
                </div>
              </div>
            ))
          ) : (
            <div className="rounded-2xl border border-dashed border-border/60 p-10 text-center text-sm text-muted-foreground">
              暂无日志
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
