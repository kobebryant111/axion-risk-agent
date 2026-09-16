import { useMemo, useRef, useState } from "react";
import {
  applyRuleOverrides,
  upsertRule,
} from "../api";
import type { RuleEditInput, RuleView } from "../data/types";
import { downloadRuleTemplate } from "../lib/excelTemplates";
import { formatInvokeError } from "../lib/fileImport";

type Props = {
  rules: RuleView[];
  onChanged: (rules: RuleView[]) => void;
  onGoSettings?: () => void;
  importRunning?: boolean;
  onStartExcelImport?: (file: File) => void;
};

const TYPE_ORDER = [
  "all",
  "common",
  "loan",
  "guarantee",
  "traffic",
  "payment",
  "data",
  "collection",
  "interbank",
  "ops",
];

function typeLabel(key: string, rules: RuleView[]) {
  if (key === "all") return "全部";
  return rules.find((r) => r.partnerType === key)?.partnerTypeLabel ?? key;
}

function levelTone(level: number) {
  if (level <= 1) return "bg-red-100 text-red-600";
  if (level === 2) return "bg-orange-100 text-orange-600";
  if (level === 3) return "bg-amber-100 text-amber-700";
  return "bg-slate-100 text-slate-500";
}

function emptyEdit(): RuleEditInput {
  return {
    id: "",
    partnerType: "loan",
    riskPoint: "",
    level: 2,
    trigger: "",
    dataSource: "",
    legalBasis: "",
    matchAnyKeywords: [],
    enabled: true,
  };
}

export function RulesPage({
  rules,
  onChanged,
  importRunning = false,
  onStartExcelImport,
}: Props) {
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [typeFilter, setTypeFilter] = useState("all");
  const [editing, setEditing] = useState<RuleEditInput | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);

  const typeKeys = useMemo(() => {
    const present = new Set(rules.map((r) => r.partnerType));
    return TYPE_ORDER.filter((k) => k === "all" || present.has(k));
  }, [rules]);

  const filtered =
    typeFilter === "all" ? rules : rules.filter((r) => r.partnerType === typeFilter);

  function startImport() {
    if (importRunning) return;
    setMsg(null);
    fileRef.current?.click();
  }

  async function toggle(rule: RuleView) {
    setBusy(true);
    setMsg(null);
    try {
      onChanged(
        await applyRuleOverrides([{ ruleId: rule.id, enabled: !rule.enabled }]),
      );
    } catch (e) {
      setMsg(formatInvokeError(e));
    } finally {
      setBusy(false);
    }
  }

  function openEdit(rule?: RuleView) {
    if (!rule) {
      setEditing(emptyEdit());
      return;
    }
    setEditing({
      id: rule.id,
      partnerType: rule.partnerType,
      riskPoint: rule.riskPoint,
      level: rule.level,
      trigger: rule.trigger || "",
      dataSource: rule.dataSource || "",
      legalBasis: rule.legalBasis || "",
      matchAnyKeywords: [...rule.matchAnyKeywords],
      enabled: rule.enabled,
    });
  }

  async function saveEdit() {
    if (!editing) return;
    setBusy(true);
    setMsg(null);
    try {
      const id =
        editing.id.trim() ||
        `CUSTOM-${editing.partnerType.toUpperCase()}-${Date.now().toString(36)}`;
      onChanged(await upsertRule({ ...editing, id }));
      setEditing(null);
      setMsg("规则已保存");
    } catch (e) {
      setMsg(formatInvokeError(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="flex h-full min-h-0 min-w-0 flex-1 flex-col gap-3 overflow-hidden">
      <header className="card flex shrink-0 flex-wrap items-center justify-between gap-3 p-4">
        <div>
          <h1 className="text-xl font-bold tracking-tight">规则库</h1>
          <div className="mt-0.5 text-xs text-axiom-muted">
            基线 {rules.filter((r) => r.source === "baseline").length} · 已编辑{" "}
            {rules.filter((r) => r.source === "edited").length} · 自定义{" "}
            {rules.filter((r) => r.source === "custom").length} · 合计 {rules.length}
            {typeFilter !== "all" ? ` · 当前筛选 ${filtered.length}` : ""}
            {" · "}先下载固定模板，按列填写后导入
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <input
            ref={fileRef}
            type="file"
            accept=".xlsx,.xls,.csv,application/vnd.openxmlformats-officedocument.spreadsheetml.sheet,application/vnd.ms-excel,text/csv"
            className="hidden"
            onChange={(e) => {
              const f = e.target.files?.[0];
              e.target.value = "";
              if (f && onStartExcelImport) onStartExcelImport(f);
            }}
          />
          <button
            type="button"
            onClick={() => downloadRuleTemplate()}
            className="rounded-full bg-white px-4 py-2 text-sm font-semibold text-axiom-text ring-1 ring-black/[0.08] hover:bg-axiom-soft"
          >
            下载模板
          </button>
          <button
            type="button"
            disabled={busy}
            onClick={() => openEdit()}
            className="rounded-full bg-axiom-accent px-4 py-2 text-sm font-semibold text-white disabled:opacity-50"
          >
            新建规则
          </button>
          <button
            type="button"
            disabled={importRunning}
            title="按固定模板全量替换当前规则库"
            onClick={startImport}
            className="rounded-full bg-white px-4 py-2 text-sm font-semibold text-axiom-accent ring-1 ring-axiom-accent/30 hover:bg-axiom-soft disabled:opacity-50"
          >
            {importRunning ? "后台导入中…" : "导入 Excel"}
          </button>
        </div>
      </header>

      {msg && (
        <div className="shrink-0 rounded-2xl bg-black/[0.03] px-4 py-2 text-sm">
          {msg}
        </div>
      )}

      <div className="flex shrink-0 flex-wrap gap-1">
        {typeKeys.map((t) => (
          <button
            key={t}
            type="button"
            onClick={() => setTypeFilter(t)}
            className={`rounded-full px-3 py-1 text-xs font-semibold ${
              typeFilter === t
                ? "bg-axiom-accent text-white"
                : "bg-black/[0.04] text-axiom-muted"
            }`}
          >
            {typeLabel(t, rules)}
          </button>
        ))}
      </div>

      <section className="card flex min-h-0 flex-1 flex-col overflow-hidden">
        <div className="min-h-0 flex-1 space-y-2 overflow-y-auto p-3">
          {filtered.length === 0 ? (
            <div className="py-16 text-center text-sm text-axiom-muted">
              暂无规则。请先下载固定模板填写后导入，或逐条新建。
            </div>
          ) : (
            filtered.map((r) => (
              <article
                key={r.id}
                className="group flex gap-3 rounded-[20px] border border-black/[0.04] bg-[#fcfbfe] px-3.5 py-3 transition hover:border-axiom-accent/20 hover:bg-white"
              >
                <div
                  className={`mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-2xl text-[11px] font-bold ${levelTone(r.level)}`}
                  title={`等级 L${r.level}`}
                >
                  L{r.level}
                </div>

                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <h3 className="text-[15px] font-semibold leading-snug text-axiom-text">
                      {r.riskPoint}
                    </h3>
                    <span className="rounded-full bg-black/[0.04] px-2 py-0.5 text-[11px] text-axiom-muted">
                      {r.partnerTypeLabel || r.partnerType}
                    </span>
                    {r.source !== "baseline" && (
                      <span className="rounded-full bg-sky-50 px-2 py-0.5 text-[11px] text-sky-700">
                        {r.source === "edited" ? "已编辑" : "自定义"}
                      </span>
                    )}
                    {!r.enabled && (
                      <span className="rounded-full bg-slate-100 px-2 py-0.5 text-[11px] text-slate-500">
                        已停用
                      </span>
                    )}
                  </div>

                  {r.trigger && (
                    <p className="mt-1 text-[12px] leading-relaxed text-axiom-muted">
                      {r.trigger}
                    </p>
                  )}

                  <div className="mt-2 flex flex-wrap gap-1.5">
                    {r.metric ? (
                      <span className="rounded-lg bg-white px-2 py-0.5 text-[11px] text-axiom-muted ring-1 ring-black/[0.04]">
                        指标 · {r.metric}
                      </span>
                    ) : (
                      r.matchAnyKeywords.slice(0, 6).map((k) => (
                        <span
                          key={k}
                          className="rounded-lg bg-white px-2 py-0.5 text-[11px] text-axiom-muted ring-1 ring-black/[0.04]"
                        >
                          {k}
                        </span>
                      ))
                    )}
                    {!r.metric && r.matchAnyKeywords.length > 6 && (
                      <span className="px-1 text-[11px] text-axiom-muted">
                        +{r.matchAnyKeywords.length - 6}
                      </span>
                    )}
                  </div>

                  <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-[11px] text-axiom-muted">
                    {r.dataSource && <span>来源 {r.dataSource}</span>}
                    {r.legalBasis && <span>依据 {r.legalBasis}</span>}
                  </div>
                </div>

                <div className="flex shrink-0 flex-col justify-center gap-1.5">
                  <button
                    type="button"
                    disabled={busy}
                    onClick={() => openEdit(r)}
                    className="rounded-full bg-white px-3 py-1.5 text-xs font-semibold text-axiom-accent ring-1 ring-black/[0.06] hover:bg-axiom-soft"
                  >
                    编辑
                  </button>
                  <button
                    type="button"
                    disabled={busy}
                    onClick={() => void toggle(r)}
                    className={`rounded-full px-3 py-1.5 text-xs font-semibold ${
                      r.enabled
                        ? "bg-emerald-50 text-emerald-700"
                        : "bg-slate-100 text-slate-500"
                    }`}
                  >
                    {r.enabled ? "启用中" : "已停用"}
                  </button>
                </div>
              </article>
            ))
          )}
        </div>
      </section>

      {editing && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/25 p-4">
          <div className="max-h-[90vh] w-full max-w-xl overflow-auto rounded-[28px] bg-white p-6 shadow-soft">
            <div className="mb-4 flex items-center justify-between">
              <h2 className="text-lg font-bold">
                {editing.id ? "编辑规则" : "新建规则"}
              </h2>
              <button type="button" onClick={() => setEditing(null)} className="text-sm">
                关闭
              </button>
            </div>
            <div className="space-y-3 text-sm">
              {editing.id ? (
                <div className="rounded-2xl bg-black/[0.03] px-3 py-2 text-[11px] text-axiom-muted">
                  系统编号 {editing.id}
                </div>
              ) : null}
              <label className="block">
                <div className="mb-1 text-xs text-axiom-muted">类型</div>
                <select
                  value={editing.partnerType}
                  onChange={(e) =>
                    setEditing({ ...editing, partnerType: e.target.value })
                  }
                  className="w-full rounded-2xl border border-black/[0.06] px-3 py-2"
                >
                  <option value="common">通用</option>
                  <option value="loan">助贷</option>
                  <option value="guarantee">融资担保</option>
                  <option value="traffic">引流</option>
                  <option value="payment">支付</option>
                  <option value="data">数据</option>
                  <option value="collection">催收</option>
                  <option value="interbank">金市同业</option>
                  <option value="ops">运营辅助</option>
                </select>
              </label>
              <Field
                label="风险点"
                value={editing.riskPoint}
                onChange={(v) => setEditing({ ...editing, riskPoint: v })}
              />
              <label className="block">
                <div className="mb-1 text-xs text-axiom-muted">等级</div>
                <select
                  value={editing.level}
                  onChange={(e) =>
                    setEditing({ ...editing, level: Number(e.target.value) })
                  }
                  className="w-full rounded-2xl border border-black/[0.06] px-3 py-2"
                >
                  {[1, 2, 3, 4].map((lv) => (
                    <option key={lv} value={lv}>
                      L{lv}
                    </option>
                  ))}
                </select>
              </label>
              <Field
                label="触发条件"
                value={editing.trigger}
                onChange={(v) => setEditing({ ...editing, trigger: v })}
              />
              <Field
                label="匹配关键词（顿号/分号分隔）"
                value={editing.matchAnyKeywords.join("、")}
                onChange={(v) =>
                  setEditing({
                    ...editing,
                    matchAnyKeywords: v
                      .split(/[、,;；|]/)
                      .map((s) => s.trim())
                      .filter(Boolean),
                  })
                }
              />
              <Field
                label="数据源"
                value={editing.dataSource}
                onChange={(v) => setEditing({ ...editing, dataSource: v })}
              />
              <Field
                label="法规依据"
                value={editing.legalBasis}
                onChange={(v) => setEditing({ ...editing, legalBasis: v })}
              />
              <label className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={editing.enabled}
                  onChange={(e) =>
                    setEditing({ ...editing, enabled: e.target.checked })
                  }
                />
                启用
              </label>
            </div>
            <div className="mt-5 flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setEditing(null)}
                className="rounded-full bg-black/[0.04] px-4 py-2 text-sm"
              >
                取消
              </button>
              <button
                type="button"
                disabled={busy || !editing.riskPoint.trim()}
                onClick={() => void saveEdit()}
                className="rounded-full bg-axiom-accent px-4 py-2 text-sm font-semibold text-white disabled:opacity-50"
              >
                保存
              </button>
            </div>
          </div>
        </div>
      )}
    </main>
  );
}

function Field({
  label,
  value,
  onChange,
  placeholder,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
}) {
  return (
    <label className="block">
      <div className="mb-1 text-xs text-axiom-muted">{label}</div>
      <input
        value={value}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
        className="w-full rounded-2xl border border-black/[0.06] px-3 py-2 outline-none focus:border-axiom-accent"
      />
    </label>
  );
}
