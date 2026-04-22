"use client";

import { useEffect, useMemo, useState } from "react";
import { RefreshCw, Search } from "lucide-react";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button, buttonVariants } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { useI18n } from "@/lib/i18n/provider";
import { AggregateApiFetchedModel } from "@/types";

interface AggregateApiModelPickerModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  aggregateApiName: string;
  models: AggregateApiFetchedModel[];
  alreadyImportedModelSlugs: Set<string>;
  selectedModelSlugs: Set<string>;
  onSelectedModelSlugsChange: (value: Set<string>) => void;
  onConfirm: () => void;
  onSyncUpstream: () => void;
  onAddManualModel: (modelSlug: string, displayName?: string) => void;
  onTestModel: (modelSlug: string) => void;
  isSyncingUpstream?: boolean;
  isSaving?: boolean;
  testingModelSlug?: string | null;
}

export function AggregateApiModelPickerModal({
  open,
  onOpenChange,
  aggregateApiName,
  models,
  alreadyImportedModelSlugs,
  selectedModelSlugs,
  onSelectedModelSlugsChange,
  onConfirm,
  onSyncUpstream,
  onAddManualModel,
  onTestModel,
  isSyncingUpstream = false,
  isSaving = false,
  testingModelSlug = null,
}: AggregateApiModelPickerModalProps) {
  const { t } = useI18n();
  const [keyword, setKeyword] = useState("");
  const [manualModelSlug, setManualModelSlug] = useState("");
  const [manualDisplayName, setManualDisplayName] = useState("");

  useEffect(() => {
    if (open) return;
    setKeyword("");
    setManualModelSlug("");
    setManualDisplayName("");
  }, [open]);

  const visibleModels = useMemo(() => {
    const normalizedKeyword = keyword.trim().toLowerCase();
    if (!normalizedKeyword) return models;
    return models.filter((model) => {
      const slug = String(model.modelSlug || "").toLowerCase();
      const displayName = String(model.displayName || "").toLowerCase();
      return slug.includes(normalizedKeyword) || displayName.includes(normalizedKeyword);
    });
  }, [keyword, models]);

  const selectedCount = selectedModelSlugs.size;
  const importedCount = alreadyImportedModelSlugs.size;

  const toggleModel = (slug: string, checked: boolean) => {
    const next = new Set(selectedModelSlugs);
    if (checked) {
      next.add(slug);
    } else {
      next.delete(slug);
    }
    onSelectedModelSlugsChange(next);
  };

  const selectVisible = () => {
    const next = new Set(selectedModelSlugs);
    for (const model of visibleModels) {
      next.add(model.modelSlug);
    }
    onSelectedModelSlugsChange(next);
  };

  const clearSelection = () => {
    onSelectedModelSlugsChange(new Set());
  };

  const addManualModel = () => {
    const slug = manualModelSlug.trim();
    if (!slug) return;
    onAddManualModel(slug, manualDisplayName.trim() || undefined);
    setManualModelSlug("");
    setManualDisplayName("");
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[90vh] w-[calc(100%-2rem)] max-w-[calc(100%-2rem)] flex-col overflow-hidden glass-card border-none sm:max-w-[760px]">
        <DialogHeader>
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0">
              <DialogTitle>{t("查看 / 管理模型")}</DialogTitle>
            </div>
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="shrink-0"
              onClick={onSyncUpstream}
              disabled={isSyncingUpstream}
            >
              <RefreshCw
                className={
                  isSyncingUpstream ? "mr-1 h-4 w-4 animate-spin" : "mr-1 h-4 w-4"
                }
              />
              {t("同步上游")}
            </Button>
          </div>
          <DialogDescription>
            {t("当前显示 {name} 的 {count} 个模型。首次为空时会自动同步，之后请按需手动同步上游。", {
              name: aggregateApiName,
              count: models.length,
            })}
          </DialogDescription>
        </DialogHeader>

        <div className="flex min-h-0 flex-1 flex-col gap-4">
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div className="relative flex-1">
              <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
              <Input
                value={keyword}
                onChange={(event) => setKeyword(event.target.value)}
                placeholder={t("搜索模型名称 / slug")}
                className="pl-9"
              />
            </div>
            <div className="flex items-center gap-2">
              <Button type="button" variant="outline" size="sm" onClick={selectVisible}>
                {t("全选可见")}
              </Button>
              <Button type="button" variant="ghost" size="sm" onClick={clearSelection}>
                {t("清空选择")}
              </Button>
            </div>
          </div>

          <div className="rounded-lg border border-border/60 bg-muted/20 px-3 py-2 text-sm text-muted-foreground">
            {t("已选择 {selected} / {total}", {
              selected: selectedCount,
              total: models.length,
            })}
          </div>

          <div className="rounded-lg border border-amber-500/20 bg-amber-500/5 px-3 py-2 text-xs text-muted-foreground">
            {t("已导入 {count} 个模型。取消勾选后，保存时会从当前聚合 API 中移除。", {
              count: importedCount,
            })}
          </div>

          <div className="rounded-lg border border-border/60 bg-muted/20 p-3">
            <div className="mb-2 text-sm font-medium">{t("手动添加模型")}</div>
            <div className="grid gap-2 sm:grid-cols-[1fr_1fr_auto]">
              <Input
                value={manualModelSlug}
                onChange={(event) => setManualModelSlug(event.target.value)}
                placeholder={t("模型 ID，例如 gpt-5.4")}
              />
              <Input
                value={manualDisplayName}
                onChange={(event) => setManualDisplayName(event.target.value)}
                placeholder={t("显示名，可选")}
              />
              <Button
                type="button"
                variant="outline"
                onClick={addManualModel}
                disabled={!manualModelSlug.trim()}
              >
                {t("添加")}
              </Button>
            </div>
            <p className="mt-2 text-xs text-muted-foreground">
              {t("用于无法从上游获取模型列表的网站；添加后可先测试模型，再保存。")}
            </p>
          </div>

          <div className="min-h-0 flex-1 overflow-y-auto overscroll-contain rounded-lg border border-border/60 bg-background/50">
            {visibleModels.length === 0 ? (
              <div className="px-4 py-8 text-center text-sm text-muted-foreground">
                {t("没有匹配的模型")}
              </div>
            ) : (
              <div className="divide-y divide-border/50">
                {visibleModels.map((model) => {
                  const label = model.displayName || model.modelSlug;
                  const checked = selectedModelSlugs.has(model.modelSlug);
                  const isAlreadyImported = alreadyImportedModelSlugs.has(model.modelSlug);
                  return (
                    <label
                      key={model.modelSlug}
                      className="flex cursor-pointer items-start gap-3 px-4 py-3 transition-colors hover:bg-muted/30"
                    >
                      <Checkbox
                        checked={checked}
                        onCheckedChange={(value) =>
                          toggleModel(model.modelSlug, Boolean(value))
                        }
                        className="mt-0.5"
                      />
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2">
                          <div className="truncate text-sm font-medium">{label}</div>
                          {isAlreadyImported ? (
                            <span className="rounded-full border border-emerald-500/20 bg-emerald-500/10 px-2 py-0.5 text-[10px] text-emerald-600">
                              {t("已导入")}
                            </span>
                          ) : null}
                        </div>
                        <div className="truncate text-xs text-muted-foreground">
                          {model.modelSlug}
                        </div>
                      </div>
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        className="shrink-0"
                        disabled={testingModelSlug === model.modelSlug}
                        onClick={(event) => {
                          event.preventDefault();
                          onTestModel(model.modelSlug);
                        }}
                      >
                        {testingModelSlug === model.modelSlug
                          ? t("测试中...")
                          : t("测试")}
                      </Button>
                    </label>
                  );
                })}
              </div>
            )}
          </div>
        </div>

        <DialogFooter>
          <DialogClose
            className={buttonVariants({ variant: "outline" })}
            disabled={isSaving}
          >
            {t("取消")}
          </DialogClose>
          <Button type="button" onClick={onConfirm} disabled={isSaving}>
            {isSaving ? t("导入中...") : t("导入已选模型")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
