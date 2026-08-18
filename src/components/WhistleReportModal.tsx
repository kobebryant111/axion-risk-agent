import type { RiskClue, WhistleReport } from "../data/types";
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
  onOpenClue?: (id: string) => void;
};

export function WhistleReportModal({ report, liveClues, onClose, onOpenClue }: Props) {
  const stats = parseReportStats(report.batchStatsJson);
  const clues = cluesForReport(stats, report.period, liveClues);
  const l12 = clues.filter((c) => c.level <= 2);
  const rest = clues.filter((c) => c.level >= 3);
  const errors = friendlySourceErrors(stats.sourceErrors ?? []);
  const isWeekly = report.kind === "weekly";

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/25 p-4">
      <div className="flex max-h-[92vh] w-full max-w-3xl flex-col overflow-hidden rounded-[28px] bg-white shadow-soft">
        <header className="flex shrink-0 items-start justify-between gap-3 border-b border-black/[0.04] px-6 py-5">
          <div>
            <div className="text-xs font-semibold text-axiom-accent">
              {isWeekly ? "周报" : "日报"} · {report.period}
            </div>
            <h2 className="mt-1 text-xl font-bold tracking-tight">{report.title}</h2>
            <div className="mt-1 text-xs text-axiom-muted">{report.createdAt}</div>
          </div>
          <button
            type="button"
            className="rounded-full bg-black/[0.04] px-3 py-1 text-sm"
            onClick={onClose}
          >
            关闭
          </button>
        </header>

        <div className="min-h-0 flex-1 space-y-5 overflow-y-auto px-6 py-5">
          <div className="grid grid-cols-2 gap-2 sm:grid-cols-5">
            <Kpi label="本期线索" value={report.clueCount} />
            <Kpi label="L1 致命" value={report.level1} hot={report.level1 > 0} />
            <Kpi label="L2 重大" value={report.level2} hot={report.level2 > 0} />
            <Kpi label="扫描机构" value={stats.partnersScanned ?? "—"} />
            <Kpi label="新增线索" value={stats.cluesUpserted ?? "—"} />
          </div>

          <p className="text-sm leading-relaxed text-axiom-text">{report.summary}</p>

          {errors.length > 0 && (
            <div className="rounded-2xl bg-amber-50 px-4 py-3 text-sm text-amber-800">
              {errors.map((e) => (
                <div key={e}>{e}</div>
              ))}
            </div>
          )}

          <section>
            <h3 className="mb-3 text-sm font-semibold">
              1 / 2 级即时处置
              <span className="ml-2 text-xs font-normal text-axiom-muted">{l12.length} 条</span>
            </h3>
            {l12.length === 0 ? (
              <div className="rounded-2xl bg-black/[0.02] px-4 py-6 text-center text-sm text-axiom-muted">
                本期无 1 / 2 级待关注线索
              </div>
            ) : (
              <div className="space-y-2">
                {l12.map((c) => (
                  <ClueCard key={c.id} clue={c} onOpen={onOpenClue} />
                ))}
              </div>
            )}
          </section>

          <section>
            <h3 className="mb-3 text-sm font-semibold">
              {isWeekly ? "3 / 4 级跟踪" : "其他等级"}
              <span className="ml-2 text-xs font-normal text-axiom-muted">{rest.length} 条</span>
            </h3>
            {rest.length === 0 ? (
              <div className="text-sm text-axiom-muted">本期无更低等级线索</div>
            ) : (
              <div className="space-y-2">
                {rest.slice(0, 20).map((c) => (
                  <ClueCard key={c.id} clue={c} onOpen={onOpenClue} compact />
                ))}
                {rest.length > 20 && (
                  <div className="text-xs text-axiom-muted">另有 {rest.length - 20} 条未展开</div>
                )}
              </div>
            )}
          </section>

          <p className="text-[11px] text-axiom-muted">
            由智联鉴控自动生成，1 / 2 级请人工复核后再关闭。
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
    <div className="rounded-2xl bg-black/[0.03] px-3 py-3">
      <div className="text-[11px] text-axiom-muted">{label}</div>
      <div className={`mt-0.5 text-xl font-bold ${hot ? "text-red-600" : ""}`}>{value}</div>
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
  onOpen?: (id: string) => void;
}) {
  return (
    <button
      type="button"
      onClick={() => onOpen?.(clue.id)}
      className="flex w-full gap-3 rounded-2xl border border-black/[0.04] bg-[#fcfbfe] px-3 py-3 text-left hover:border-axiom-accent/20 hover:bg-white"
    >
      <div
        className={`mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-2xl text-[11px] font-bold ${levelTone(clue.level)}`}
      >
        L{clue.level}
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          <span className="truncate text-sm font-semibold">{clue.title}</span>
          <span className="rounded-full bg-black/[0.04] px-2 py-0.5 text-[11px] text-axiom-muted">
            {clue.partner}
          </span>
        </div>
        {!compact && clue.summary && (
          <p className="mt-1 line-clamp-2 text-[12px] leading-relaxed text-axiom-muted">
            {clue.summary}
          </p>
        )}
        <div className="mt-1.5 flex flex-wrap gap-x-3 text-[11px] text-axiom-muted">
          <span>{sourceLabel(clue.sourceSystem)}</span>
          {clue.credibility && <span>{clue.credibility} 类源</span>}
          <span>{clue.eventDate}</span>
          <span>{clue.status}</span>
        </div>
      </div>
    </button>
  );
}
