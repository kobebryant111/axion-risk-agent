import { useEffect, useState } from "react";
import type { RiskClue, WhistleReport } from "../data/types";
import { downloadWhistleAlerts } from "../lib/excelTemplates";
import {
  cluesForReport,
  friendlySourceErrors,
  levelTone,
  parseReportStats,
  sourceLabel,
  type ReportClue,
} from "../lib/whistleUi";

type Props = {
  report: WhistleReport;
  liveClues: RiskClue[];
  onClose: () => void;
  onOpenClue?: (clue: ReportClue) => void;
};

export function WhistleReportModal({ report, liveClues, onClose, onOpenClue }: Props) {
  const stats = parseReportStats(report.batchStatsJson);
  const clues = cluesForReport(stats, report.period, liveClues);
  const l12 = clues.filter((c) => c.level <= 2);
  const rest = clues.filter((c) => c.level >= 3);
  const errors = friendlySourceErrors(stats.sourceErrors ?? []);
  const isWeekly = report.kind === "weekly";
  const kindLabel = isWeekly ? "周报" : "日报";
  const [exportTip, setExportTip] = useState<string | null>(null);

  useEffect(() => {
    if (!exportTip) return;
    const t = window.setTimeout(() => setExportTip(null), 3500);
    return () => window.clearTimeout(t);
  }, [exportTip]);

  function exportAlerts() {
    const ordered = [...l12, ...rest];
    try {
      downloadWhistleAlerts({
        kindLabel,
        period: report.period,
        title: report.title,
        createdAt: report.createdAt,
        summary: report.summary,
        clues: ordered,
        sourceLabel,
      });
      setExportTip(
        ordered.length > 0
          ? `已导出 ${ordered.length} 条预警事项，请查看下载的 Excel 文件`
          : "已生成导出文件（本期暂无预警事项）",
      );
    } catch (e) {
      setExportTip(
        e instanceof Error ? `导出失败：${e.message}` : "导出失败，请重试",
      );
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/35 p-4">
      <div className="flex max-h-[92vh] w-full max-w-3xl flex-col overflow-hidden rounded-[28px] bg-white shadow-soft">
        <header className="flex shrink-0 items-start justify-between gap-3 border-b border-black/[0.06] px-6 py-5">
          <div>
            <div className="text-sm font-bold text-axiom-accent">
              {kindLabel} · {report.period}
            </div>
            <h2 className="mt-1 text-2xl font-bold tracking-tight text-axiom-text">
              {report.title}
            </h2>
            <div className="mt-1.5 text-sm font-semibold text-axiom-muted">
              {report.createdAt}
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            <button
              type="button"
              className={`rounded-full px-4 py-1.5 text-sm font-semibold text-white hover:opacity-90 ${
                exportTip && !exportTip.startsWith("导出失败")
                  ? "bg-emerald-600"
                  : "bg-axiom-accent"
              }`}
              onClick={exportAlerts}
              title="导出本报告预警事项为 Excel"
            >
              {exportTip && !exportTip.startsWith("导出失败") ? "已导出" : "导出"}
            </button>
            <button
              type="button"
              className="rounded-full bg-black/[0.06] px-4 py-1.5 text-sm font-semibold text-axiom-text"
              onClick={onClose}
            >
              关闭
            </button>
          </div>
        </header>

        {exportTip && (
          <div
            className={`shrink-0 border-b px-6 py-2.5 text-sm font-semibold ${
              exportTip.startsWith("导出失败")
                ? "border-red-100 bg-red-50 text-red-700"
                : "border-emerald-100 bg-emerald-50 text-emerald-800"
            }`}
            role="status"
          >
            {exportTip}
          </div>
        )}

        <div className="min-h-0 flex-1 space-y-5 overflow-y-auto px-6 py-5">
          <div className="grid grid-cols-2 gap-2.5 sm:grid-cols-5">
            <Kpi label="本次命中" value={report.clueCount} />
            <Kpi label="L1 致命" value={report.level1} hot={report.level1 > 0} />
            <Kpi label="L2 重大" value={report.level2} hot={report.level2 > 0} />
            <Kpi label="扫描机构" value={stats.partnersScanned ?? "—"} />
            <Kpi label="原始命中" value={stats.rawHits ?? "—"} />
          </div>

          <p className="text-base font-medium leading-relaxed text-axiom-text">
            {report.summary}
          </p>
          {stats.scopeNote && (
            <p className="text-sm font-semibold text-axiom-muted">
              监测范围：{stats.scopeNote}
            </p>
          )}

          <section>
            <h3 className="mb-2 text-base font-bold text-axiom-text">
              全网检索词
              {(stats.searchQueries ?? []).length > 0
                ? `（${(stats.searchQueries ?? []).length} 条）`
                : ""}
            </h3>
            {(stats.searchQueries ?? []).length > 0 ? (
              <div className="max-h-56 space-y-2 overflow-y-auto">
                {(stats.searchQueries ?? []).map((q, i) => (
                  <div
                    key={`${i}-${q}`}
                    className="break-all rounded-2xl bg-black/[0.04] px-3.5 py-3 font-mono text-sm font-medium leading-relaxed text-axiom-text"
                  >
                    {q}
                  </div>
                ))}
              </div>
            ) : (
              <div className="rounded-2xl bg-amber-50 px-4 py-3 text-sm font-semibold text-amber-900">
                本报告未记录检索词。请重新点「出周报 / 出日报」生成新报告后即可看到实际发出的搜索词。
              </div>
            )}
          </section>

          {errors.length > 0 && (
            <div className="rounded-2xl bg-amber-50 px-4 py-3 text-sm font-semibold text-amber-900">
              {errors.map((e) => (
                <div key={e}>{e}</div>
              ))}
            </div>
          )}

          <section>
            <h3 className="mb-3 text-base font-bold text-axiom-text">
              1 / 2 级即时处置
              <span className="ml-2 text-sm font-semibold text-axiom-muted">
                {l12.length} 条
              </span>
            </h3>
            {l12.length === 0 ? (
              <div className="rounded-2xl bg-black/[0.03] px-4 py-6 text-center text-base font-semibold text-axiom-muted">
                本期无 1 / 2 级待关注线索
              </div>
            ) : (
              <div className="space-y-2.5">
                {l12.map((c, i) => (
                  <ClueCard key={c.id || `${c.title}-${i}`} clue={c} onOpen={onOpenClue} />
                ))}
              </div>
            )}
          </section>

          <section>
            <h3 className="mb-3 text-base font-bold text-axiom-text">
              {isWeekly ? "3 / 4 级跟踪" : "其他等级"}
              <span className="ml-2 text-sm font-semibold text-axiom-muted">
                {rest.length} 条
              </span>
            </h3>
            {rest.length === 0 ? (
              <div className="text-base font-semibold text-axiom-muted">
                本期无更低等级线索
              </div>
            ) : (
              <div className="space-y-2.5">
                {rest.slice(0, 20).map((c, i) => (
                  <ClueCard key={c.id || `${c.title}-${i}`} clue={c} onOpen={onOpenClue} compact />
                ))}
                {rest.length > 20 && (
                  <div className="text-sm font-semibold text-axiom-muted">
                    另有 {rest.length - 20} 条未展开
                  </div>
                )}
              </div>
            )}
          </section>

          <p className="text-sm font-semibold text-axiom-muted">
            由智鉴风控官自动生成，1 / 2 级请人工复核后再关闭。
          </p>
        </div>
      </div>
    </div>
  );
}

function Kpi({
  label,
  value,
  hot,
}: {
  label: string;
  value: number | string;
  hot?: boolean;
}) {
  return (
    <div className="rounded-2xl bg-black/[0.04] px-3 py-3.5">
      <div className="text-sm font-bold text-axiom-muted">{label}</div>
      <div
        className={`mt-1 text-2xl font-bold tracking-tight ${hot ? "text-red-600" : "text-axiom-text"}`}
      >
        {value}
      </div>
    </div>
  );
}

function ClueCard({
  clue,
  compact,
  onOpen,
}: {
  clue: ReportClue;
  compact?: boolean;
  onOpen?: (clue: ReportClue) => void;
}) {
  return (
    <button
      type="button"
      onClick={() => onOpen?.(clue)}
      className="flex w-full gap-3 rounded-2xl border border-black/[0.06] bg-[#fcfbfe] px-3.5 py-3.5 text-left hover:border-axiom-accent/25 hover:bg-white"
    >
      <div
        className={`mt-0.5 flex h-10 w-10 shrink-0 items-center justify-center rounded-2xl text-sm font-bold ${levelTone(clue.level)}`}
      >
        L{clue.level}
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          <span className="truncate text-base font-bold text-axiom-text">{clue.title}</span>
          <span className="rounded-full bg-black/[0.06] px-2.5 py-0.5 text-xs font-bold text-axiom-muted">
            {clue.partner}
          </span>
        </div>
        {!compact && clue.summary && (
          <p className="mt-1.5 line-clamp-3 text-sm font-medium leading-relaxed text-axiom-muted">
            {clue.summary}
          </p>
        )}
        <div className="mt-2 flex flex-wrap gap-x-3 gap-y-1 text-xs font-bold text-axiom-muted">
          <span>{sourceLabel(clue.sourceSystem)}</span>
          {clue.credibility && <span>{clue.credibility} 类源</span>}
          <span>{clue.eventDate}</span>
          <span>{clue.status}</span>
        </div>
      </div>
    </button>
  );
}
