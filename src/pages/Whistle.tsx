import { useEffect, useMemo, useState } from "react";
import { flushSync } from "react-dom";
import {
  getWhistleSchedule,
  listWhistleReports,
  runWhistleJob,
  saveWhistleLastScope,
  saveWhistleSchedule,
  updateClue,
} from "../api";
import {
  PartnerScopePicker,
  effectivePartnerIds,
  scopeSummary,
  type PartnerScope,
} from "../components/PartnerScopePicker";
import { FancySelect } from "../components/FancySelect";
import { WhistleReportModal } from "../components/WhistleReportModal";
import type {
  BatchResult,
  Partner,
  RiskClue,
  WhistleReport,
  WhistleSchedule,
} from "../data/types";
import { friendlySourceErrors, levelTone, resolveLiveClue, sourceLabel } from "../lib/whistleUi";

type Props = {
  clues: RiskClue[];
  partners: Partner[];
  onCluesChanged: (clues: RiskClue[]) => void;
  onRefreshAll: () => Promise<void>;
};

const WEEKDAYS = [
  { v: 1, label: "周一" },
  { v: 2, label: "周二" },
  { v: 3, label: "周三" },
  { v: 4, label: "周四" },
  { v: 5, label: "周五" },
  { v: 6, label: "周六" },
  { v: 7, label: "周日" },
];

function todayYmd() {
  const d = new Date();
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function pad2(n: number) {
  return String(n).padStart(2, "0");
}

function ymdFromDate(d: Date) {
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
}

/** 该日所在周一至周日（与后端自然周一致） */
function weekRangeHint(ymd: string) {
  const [y, m, d] = ymd.split("-").map(Number);
  if (!y || !m || !d) return ymd;
  const date = new Date(y, m - 1, d);
  const dow = (date.getDay() + 6) % 7;
  const start = new Date(date);
  start.setDate(date.getDate() - dow);
  const end = new Date(start);
  end.setDate(start.getDate() + 6);
  return `${ymdFromDate(start)} ~ ${ymdFromDate(end)}`;
}

function scopeFromSchedule(
  s: WhistleSchedule,
  partners: Partner[],
): PartnerScope {
  const known = new Set(partners.map((p) => p.id));
  const keep = (ids?: string[]) => {
    const raw = (ids ?? []).filter(Boolean);
    if (partners.length === 0) return raw;
    return raw.filter((id) => known.has(id));
  };
  if (s.lastScopeMode === "all") return { mode: "all", ids: [] };
  if (s.lastScopeMode === "selected") {
    const ids = keep(s.lastScopeIds);
    if (ids.length > 0) return { mode: "selected", ids };
  }
  const ids = keep(s.partnerIds);
  return ids.length > 0 ? { mode: "selected", ids } : { mode: "all", ids: [] };
}

export function WhistlePage({
  clues,
  partners,
  onCluesChanged,
  onRefreshAll,
}: Props) {
  const partnerCount = partners.length;
  const [saving, setSaving] = useState(false);
  const [runningKind, setRunningKind] = useState<"daily" | "weekly" | null>(null);
  const busy = saving || runningKind !== null;
  const [batch, setBatch] = useState<BatchResult | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [levelFilter, setLevelFilter] = useState<number | "all">("all");
  const [selected, setSelected] = useState<RiskClue | null>(null);
  const [schedule, setSchedule] = useState<WhistleSchedule | null>(null);
  const [dailyEnabled, setDailyEnabled] = useState(true);
  const [dailyTime, setDailyTime] = useState("08:00");
  const [weeklyEnabled, setWeeklyEnabled] = useState(true);
  const [weeklyDow, setWeeklyDow] = useState(1);
  const [weeklyTime, setWeeklyTime] = useState("09:00");
  const [reports, setReports] = useState<WhistleReport[]>([]);
  const [viewReport, setViewReport] = useState<WhistleReport | null>(null);
  const [scope, setScope] = useState<PartnerScope>({ mode: "all", ids: [] });
  const [runDate, setRunDate] = useState(todayYmd);

  async function refreshReports() {
    try {
      setReports(await listWhistleReports(40));
    } catch {
      /* browser demo */
    }
  }

  async function refreshSchedule(syncScope = false) {
    try {
      const s = await getWhistleSchedule();
      setSchedule(s);
      setDailyEnabled(!!s.dailyEnabled);
      setDailyTime(s.dailyTime || "08:00");
      setWeeklyEnabled(!!s.weeklyEnabled);
      setWeeklyDow(s.weeklyDow || 1);
      setWeeklyTime(s.weeklyTime || "09:00");
      if (syncScope) {
        setScope(scopeFromSchedule(s, partners));
      }
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    }
  }

  useEffect(() => {
    void refreshSchedule(true);
    void refreshReports();
  }, []);

  useEffect(() => {
    if (partners.length === 0) return;
    setScope((prev) => {
      if (prev.mode !== "selected") return prev;
      const known = new Set(partners.map((p) => p.id));
      const ids = prev.ids.filter((id) => known.has(id));
      if (ids.length === prev.ids.length) return prev;
      return ids.length > 0 ? { mode: "selected", ids } : prev;
    });
  }, [partners]);

  const filtered = useMemo(() => {
    if (levelFilter === "all") return clues;
    return clues.filter((c) => c.level === levelFilter);
  }, [clues, levelFilter]);

  const alerts = useMemo(
    () =>
      clues.filter(
        (c) => c.level <= 2 && (c.status.includes("待") || c.status.includes("复核")),
      ),
    [clues],
  );

  const canRun = partnerCount > 0 && (scope.mode === "all" || scope.ids.length > 0);
  const selectedIds = effectivePartnerIds(scope);

  function changeScope(next: PartnerScope) {
    setScope(next);
    void saveWhistleLastScope(next.mode, next.ids);
  }

  async function saveSchedule() {
    if (scope.mode === "selected" && scope.ids.length === 0) {
      setErr("指定机构模式下请至少勾选一家，或改回「全部机构」");
      return;
    }
    setSaving(true);
    setErr(null);
    setMsg(null);
    try {
      const s = await saveWhistleSchedule({
        dailyEnabled,
        dailyTime,
        weeklyEnabled,
        weeklyDow,
        weeklyTime,
        partnerIds: selectedIds,
      });
      setSchedule(s);
      setDailyTime(s.dailyTime);
      setWeeklyTime(s.weeklyTime);
      setMsg(`定时已保存 · ${s.scopeHint || scopeSummary(scope, partnerCount)}`);
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  }

  async function runJob(kind: "daily" | "weekly") {
    flushSync(() => {
      setRunningKind(kind);
      setErr(null);
      setMsg(null);
    });
    try {
      const res = await runWhistleJob(kind, selectedIds, runDate);
      setBatch(res.batch);
      setViewReport(res.report);
      setMsg(`${kind === "daily" ? "日报" : "周报"}已生成：${res.report.title}`);
      await onRefreshAll();
      await refreshReports();
      await refreshSchedule();
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setRunningKind(null);
    }
  }

  async function changeLevel(clue: RiskClue, level: number) {
    try {
      const updated = await updateClue({
        clueId: clue.id,
        level,
        actor: "风控同学",
      });
      onCluesChanged(clues.map((c) => (c.id === updated.id ? { ...c, ...updated } : c)));
      setSelected({ ...clue, ...updated });
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    }
  }

  async function closeClue(clue: RiskClue) {
    try {
      const updated = await updateClue({
        clueId: clue.id,
        status: "已关闭",
        actor: "风控同学",
      });
      onCluesChanged(clues.map((c) => (c.id === updated.id ? { ...c, ...updated } : c)));
      setSelected({ ...clue, ...updated });
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    }
  }

  return (
    <main className="flex min-w-0 flex-1 flex-col gap-3 overflow-auto">
      <header className="card flex flex-wrap items-center justify-between gap-3 px-5 py-4">
        <div>
          <div className="text-xs font-semibold text-axiom-accent">风险吹哨</div>
          <h1 className="mt-0.5 text-xl font-bold tracking-tight">监测跑批</h1>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <label className="flex items-center gap-2 rounded-full bg-black/[0.04] py-1 pl-3 pr-2">
            <span className="text-xs text-axiom-muted">日期</span>
            <input
              type="date"
              value={runDate}
              max={todayYmd()}
              disabled={busy}
              onChange={(e) => setRunDate(e.target.value || todayYmd())}
              className="rounded-full border-0 bg-transparent px-1 py-1 text-sm outline-none"
            />
          </label>
          <button
            type="button"
            disabled={busy || !canRun}
            onClick={() => void runJob("daily")}
            className="rounded-full bg-axiom-accent px-4 py-2 text-sm font-semibold text-white disabled:opacity-50"
          >
            {runningKind === "daily" ? "生成中…" : "出日报"}
          </button>
          <button
            type="button"
            disabled={busy || !canRun}
            onClick={() => void runJob("weekly")}
            className="rounded-full bg-white px-4 py-2 text-sm font-semibold text-axiom-text ring-1 ring-black/[0.08] disabled:opacity-50"
          >
            {runningKind === "weekly" ? "生成中…" : "出周报"}
          </button>
        </div>
      </header>

      <section className="card space-y-4 p-5">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="text-sm font-semibold">跑批设置</div>
          <button
            type="button"
            disabled={busy}
            onClick={() => void saveSchedule()}
            className="rounded-full bg-axiom-accent px-3.5 py-1.5 text-xs font-semibold text-white disabled:opacity-50"
          >
            保存定时
          </button>
        </div>

        <div className="grid gap-3 sm:grid-cols-2">
          <div className="flex items-center gap-3 rounded-2xl bg-black/[0.02] px-3 py-2.5">
            <input
              type="checkbox"
              checked={dailyEnabled}
              onChange={(e) => setDailyEnabled(e.target.checked)}
              className="h-4 w-4 accent-axiom-accent"
            />
            <span className="text-sm font-medium">日报</span>
            <input
              type="time"
              value={dailyTime}
              onChange={(e) => setDailyTime(e.target.value)}
              className="ml-auto rounded-2xl border border-black/[0.05] bg-gradient-to-b from-white to-[#faf9fc] px-3 py-1.5 text-sm shadow-[inset_0_1px_0_rgba(255,255,255,0.8)] outline-none focus:border-axiom-accent/40 focus:ring-2 focus:ring-axiom-accent/15"
            />
          </div>
          <div className="flex items-center gap-2 rounded-2xl bg-black/[0.02] px-3 py-2.5">
            <input
              type="checkbox"
              checked={weeklyEnabled}
              onChange={(e) => setWeeklyEnabled(e.target.checked)}
              className="h-4 w-4 accent-axiom-accent"
            />
            <span className="shrink-0 text-sm font-medium">周报</span>
            <FancySelect
              compact
              className="w-[6.75rem] shrink-0"
              value={String(weeklyDow)}
              options={WEEKDAYS.map((d) => ({ value: String(d.v), label: d.label }))}
              onChange={(v) => setWeeklyDow(Number(v))}
            />
            <input
              type="time"
              value={weeklyTime}
              onChange={(e) => setWeeklyTime(e.target.value)}
              className="ml-auto rounded-2xl border border-black/[0.05] bg-gradient-to-b from-white to-[#faf9fc] px-3 py-1.5 text-sm shadow-[inset_0_1px_0_rgba(255,255,255,0.8)] outline-none focus:border-axiom-accent/40 focus:ring-2 focus:ring-axiom-accent/15"
            />
          </div>
        </div>
        {schedule && (
          <div className="text-[11px] text-axiom-muted">
            {schedule.nextDailyHint}
            {schedule.lastDailyRun ? ` · 上次日报 ${schedule.lastDailyRun}` : ""}
            {"  ·  "}
            {schedule.nextWeeklyHint}
          </div>
        )}

        <div className="border-t border-black/[0.04] pt-4">
          <div className="mb-3 text-[11px] leading-relaxed text-axiom-muted">
            手动跑批按上方日期检索：日报 {runDate}；周报 {weekRangeHint(runDate)}。
            机构范围如下（可指定公司）；定时任务仍按滚动 24 小时 / 7 天。
          </div>
          <PartnerScopePicker
            partners={partners}
            value={scope}
            onChange={changeScope}
            disabled={busy}
          />
        </div>
      </section>

      {partnerCount === 0 && (
        <div className="rounded-2xl bg-amber-50 px-4 py-2.5 text-sm text-amber-800">
          请先在「机构名单」导入合作机构。
        </div>
      )}
      {scope.mode === "selected" && scope.ids.length === 0 && partnerCount > 0 && (
        <div className="rounded-2xl bg-amber-50 px-4 py-2.5 text-sm text-amber-800">
          请先选择机构，或改回监测全部。
        </div>
      )}
      {err && (
        <div className="rounded-2xl bg-red-50 px-4 py-2.5 text-sm text-red-600">{err}</div>
      )}
      {msg && (
        <div className="rounded-2xl bg-emerald-50 px-4 py-2.5 text-sm text-emerald-700">{msg}</div>
      )}
      {batch && (
        <section className="card px-5 py-4">
          <div className="mb-2 text-xs font-semibold text-axiom-muted">
            本次结果{batch.scopeNote ? ` · ${batch.scopeNote}` : ""}
          </div>
          <div className="flex flex-wrap gap-x-5 gap-y-1 text-sm">
            <span>
              扫描 <b>{batch.partnersScanned}</b>
            </span>
            <span>
              命中 <b>{batch.rawHits}</b>
            </span>
            <span>
              新增 <b>{batch.cluesUpserted}</b>
            </span>
            <span className={batch.level1 ? "text-red-600" : ""}>
              L1 <b>{batch.level1}</b>
            </span>
            <span className={batch.level2 ? "text-orange-600" : ""}>
              L2 <b>{batch.level2}</b>
            </span>
          </div>
          {friendlySourceErrors(batch.sourceErrors).map((e) => (
            <div key={e} className="mt-2 text-xs text-amber-700">
              {e}
            </div>
          ))}
          {(batch.searchQueries ?? []).length > 0 && (
            <div className="mt-3 space-y-1">
              <div className="text-[11px] font-semibold text-axiom-muted">
                本次检索词（{(batch.searchQueries ?? []).length} 条）
              </div>
              <div className="max-h-48 space-y-1 overflow-y-auto">
              {(batch.searchQueries ?? []).map((q) => (
                <div
                  key={q}
                  className="break-all rounded-xl bg-black/[0.03] px-3 py-2 font-mono text-[11px] leading-relaxed text-axiom-text/80"
                >
                  {q}
                </div>
              ))}
              </div>
            </div>
          )}
        </section>
      )}

      <section className="card p-5">
        <div className="mb-4 flex items-center justify-between gap-3">
          <div className="text-base font-semibold">日报 / 周报</div>
          <button
            type="button"
            className="text-xs font-semibold text-axiom-accent"
            onClick={() => void refreshReports()}
          >
            刷新
          </button>
        </div>
        {reports.length === 0 ? (
          <div className="py-8 text-center text-sm text-axiom-muted">
            暂无报告。选择机构后点「出日报」或「出周报」。
          </div>
        ) : (
          <div className="space-y-2">
            {reports.map((r) => (
              <button
                key={r.id}
                type="button"
                onClick={() => setViewReport(r)}
                className="flex w-full items-center gap-3 rounded-2xl border border-black/[0.03] px-3 py-3 text-left hover:bg-black/[0.015]"
              >
                <span
                  className={`rounded-full px-2 py-0.5 text-[11px] font-bold ${
                    r.kind === "weekly"
                      ? "bg-violet-100 text-violet-700"
                      : "bg-sky-100 text-sky-700"
                  }`}
                >
                  {r.kind === "weekly" ? "周报" : "日报"}
                </span>
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm font-semibold">{r.title}</div>
                  <div className="mt-0.5 truncate text-[11px] text-axiom-muted">
                    {r.summary}
                  </div>
                </div>
                <div className="shrink-0 text-right text-[11px] text-axiom-muted">
                  <div>
                    L1 {r.level1} · L2 {r.level2}
                  </div>
                  <div>{r.createdAt}</div>
                </div>
              </button>
            ))}
          </div>
        )}
      </section>

      {alerts.length > 0 && (
        <section className="flex flex-wrap gap-2 rounded-[22px] border border-red-100 bg-red-50/60 px-4 py-3">
          <span className="text-sm font-semibold text-red-700">
            待处置 {alerts.length}
          </span>
          {alerts.slice(0, 3).map((a) => (
            <button
              key={a.id}
              type="button"
              onClick={() => setSelected(a)}
              className="truncate rounded-full bg-white px-3 py-1 text-xs shadow-soft"
            >
              L{a.level} {a.partner}
            </button>
          ))}
        </section>
      )}

      <section className="card p-5">
        <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
          <div className="text-base font-semibold">标准化风险线索</div>
          <div className="flex gap-1">
            {(["all", 1, 2, 3, 4] as const).map((lv) => (
              <button
                key={String(lv)}
                type="button"
                onClick={() => setLevelFilter(lv)}
                className={`rounded-full px-3 py-1 text-xs font-semibold ${
                  levelFilter === lv
                    ? "bg-axiom-accent text-white"
                    : "bg-black/[0.04] text-axiom-muted"
                }`}
              >
                {lv === "all" ? "全部" : `L${lv}`}
              </button>
            ))}
          </div>
        </div>

        <div className="space-y-3">
          {filtered.length === 0 ? (
            <div className="py-12 text-center text-sm text-axiom-muted">
              暂无线索。选择机构后点「出日报 / 出周报」，或等待定时任务。
            </div>
          ) : (
            filtered.map((c) => (
              <button
                key={c.id}
                type="button"
                onClick={() => setSelected(c)}
                className="flex w-full items-center gap-4 rounded-2xl border border-black/[0.03] px-3 py-3 text-left hover:bg-black/[0.015]"
              >
                <div
                  className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-2xl text-xs font-bold ${levelTone(
                    c.level,
                  )}`}
                >
                  L{c.level}
                </div>
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm font-semibold">{c.title}</div>
                  <div className="mt-0.5 flex flex-wrap gap-x-3 text-[11px] text-axiom-muted">
                    <span>{c.partner}</span>
                    <span>{c.partnerType}</span>
                    <span>{c.eventDate}</span>
                    <span className="font-medium text-axiom-text/70">
                      {sourceLabel(c.sourceSystem)}
                    </span>
                    <span>{c.credibility ?? "—"} 类源</span>
                    {c.relatedPartyFlag && (
                      <span className="text-orange-600">关联方传导</span>
                    )}
                    {c.denoiseStatus === "repeat" && <span>重复折叠</span>}
                  </div>
                </div>
                <div className="shrink-0 text-xs text-axiom-muted">{c.status}</div>
              </button>
            ))
          )}
        </div>
      </section>

      {selected && (
        <div className="fixed inset-0 z-[60] flex justify-end bg-black/20 p-4">
          <div className="flex h-full w-full max-w-md flex-col rounded-[28px] bg-white shadow-soft">
            <div className="flex items-start justify-between gap-3 border-b border-black/[0.04] p-5">
              <div>
                <div
                  className={`inline-flex rounded-full px-2 py-0.5 text-xs font-bold ${levelTone(selected.level)}`}
                >
                  L{selected.level}
                </div>
                <h2 className="mt-2 text-lg font-bold leading-snug">{selected.title}</h2>
              </div>
              <button
                type="button"
                className="rounded-full bg-black/[0.04] px-3 py-1 text-sm"
                onClick={() => setSelected(null)}
              >
                关闭
              </button>
            </div>
            <div className="flex-1 space-y-3 overflow-auto p-5 text-sm">
              <Row k="机构" v={selected.partner} />
              <Row k="摘要" v={selected.summary || "—"} />
              <Row
                k="规则"
                v={`${selected.ruleId || "—"} @ ${selected.ruleSetVersion || "—"}`}
              />
              <Row k="法规依据" v={selected.legalBasis || "—"} />
              <Row
                k="来源"
                v={`${sourceLabel(selected.sourceSystem)}（${selected.sourceSystem || "—"}） · ${selected.credibility || "—"}`}
              />
              <Row k="证据链接" v={selected.sourceUrl || "—"} url={selected.sourceUrl} />
              <Row k="降噪" v={selected.denoiseStatus || "—"} />
              <Row k="状态" v={selected.status} />
            </div>
            <div className="space-y-2 border-t border-black/[0.04] p-5">
              <div className="text-xs text-axiom-muted">人工改级（留痕审计）</div>
              <div className="flex flex-wrap gap-2">
                {[1, 2, 3, 4].map((lv) => (
                  <button
                    key={lv}
                    type="button"
                    onClick={() => void changeLevel(selected, lv)}
                    className="rounded-full bg-axiom-soft px-3 py-1.5 text-xs font-semibold text-axiom-accent"
                  >
                    改为 L{lv}
                  </button>
                ))}
                <button
                  type="button"
                  onClick={() => void closeClue(selected)}
                  className="rounded-full bg-axiom-accent px-3 py-1.5 text-xs font-semibold text-white"
                >
                  人工关闭
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {runningKind && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/25 p-4">
          <div className="w-full max-w-sm rounded-[28px] bg-white px-6 py-8 text-center shadow-soft">
            <div className="mx-auto h-10 w-10 animate-spin rounded-full border-2 border-axiom-soft border-t-axiom-accent" />
            <div className="mt-4 text-base font-bold">
              正在生成{runningKind === "daily" ? "日报" : "周报"}
            </div>
            <div className="mt-1.5 text-xs leading-relaxed text-axiom-muted">
              {runningKind === "daily"
                ? `全网检索 ${runDate}（自然日）`
                : `全网检索 ${weekRangeHint(runDate)}（该日所在自然周）`}
              ，完成后会自动打开报告。
            </div>
          </div>
        </div>
      )}

      {viewReport && (
        <WhistleReportModal
          report={viewReport}
          liveClues={clues}
          onClose={() => setViewReport(null)}
          onOpenClue={(snap) => {
            setViewReport(null);
            setSelected(resolveLiveClue(clues, snap));
          }}
        />
      )}
    </main>
  );
}

function Row({ k, v, url }: { k: string; v: string; url?: string }) {
  const href = url?.trim();
  const isLink = Boolean(href && /^https?:\/\//i.test(href));
  return (
    <div>
      <div className="text-xs text-axiom-muted">{k}</div>
      {isLink ? (
        <a
          href={href}
          target="_blank"
          rel="noreferrer"
          className="mt-0.5 block break-all font-medium text-axiom-accent underline-offset-2 hover:underline"
        >
          {href}
        </a>
      ) : (
        <div className="mt-0.5 break-all font-medium text-axiom-text">{v}</div>
      )}
    </div>
  );
}
