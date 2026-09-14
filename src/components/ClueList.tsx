import type { RiskClue } from "../data/types";

function levelTone(level: number) {
  if (level <= 1) return "bg-red-100 text-red-600";
  if (level === 2) return "bg-orange-100 text-orange-600";
  if (level === 3) return "bg-amber-100 text-amber-700";
  return "bg-slate-100 text-slate-500";
}

type Props = {
  clues: RiskClue[];
  onOpenClue?: (clue: RiskClue) => void;
  onViewAll?: () => void;
};

export function ClueList({ clues, onOpenClue, onViewAll }: Props) {
  return (
    <div className="card p-5">
      <div className="mb-4 flex items-center justify-between">
        <div>
          <div className="text-base font-semibold">最新风险线索</div>
          <div className="text-sm font-medium text-axiom-muted">默认展示昨日新增</div>
        </div>
        <button
          type="button"
          onClick={() => onViewAll?.()}
          className="rounded-full bg-axiom-soft px-3 py-1.5 text-xs font-semibold text-axiom-accent hover:bg-axiom-accent/15"
        >
          查看全部
        </button>
      </div>
      <div className="space-y-3">
        {clues.length === 0 ? (
          <div className="py-8 text-center text-sm text-axiom-muted">暂无线索</div>
        ) : (
          clues.map((c) => (
            <button
              key={c.id}
              type="button"
              onClick={() => onOpenClue?.(c)}
              className="flex w-full items-center gap-4 rounded-2xl border border-black/[0.03] px-3 py-3 text-left transition hover:border-axiom-accent/20 hover:bg-black/[0.015]"
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
                <div className="mt-0.5 flex flex-wrap gap-x-3 text-xs font-medium text-axiom-muted">
                  <span>{c.partner}</span>
                  <span>{c.partnerType}</span>
                  <span>{c.eventDate}</span>
                  <span>{c.owner}</span>
                </div>
                <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-black/[0.05]">
                  <div
                    className="h-full rounded-full bg-axiom-accent"
                    style={{ width: `${c.progress}%` }}
                  />
                </div>
              </div>
              <div className="shrink-0 text-right">
                <div className="mb-2 text-xs font-medium text-axiom-muted">{c.status}</div>
                <span className="inline-flex rounded-full bg-axiom-accent px-3 py-1.5 text-xs font-semibold text-white">
                  {c.status === "已关闭"
                    ? "已关闭"
                    : c.progress > 0 || c.status === "跟踪中"
                      ? "继续"
                      : "开始"}
                </span>
              </div>
            </button>
          ))
        )}
      </div>
    </div>
  );
}
