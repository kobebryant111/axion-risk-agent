import type { EventItem } from "../data/types";

function levelDot(level: number) {
  if (level <= 1) return "bg-red-500";
  if (level === 2) return "bg-orange-500";
  if (level === 3) return "bg-amber-400";
  return "bg-slate-300";
}

export function RightPanel({
  events,
  monitorDate,
  onOpenEvent,
}: {
  events: EventItem[];
  monitorDate: string;
  onOpenEvent?: (event: EventItem) => void;
}) {
  const day = Number(monitorDate.split("-")[2] ?? "27");
  const days = Array.from({ length: 31 }, (_, i) => i + 1);

  return (
    <aside className="flex w-[280px] shrink-0 flex-col gap-4">
      <div className="card p-4">
        <div className="mb-3 flex items-center justify-between">
          <div className="text-sm font-semibold">2026 年 7 月</div>
          <div className="text-sm font-medium text-axiom-muted">监测日历</div>
        </div>
        <div className="grid grid-cols-7 gap-1 text-center text-xs font-medium text-axiom-muted">
          {["一", "二", "三", "四", "五", "六", "日"].map((d) => (
            <div key={d} className="py-1">
              {d}
            </div>
          ))}
          {Array.from({ length: 2 }).map((_, i) => (
            <div key={`pad-${i}`} />
          ))}
          {days.map((d) => {
            const active = d === day;
            return (
              <div
                key={d}
                className={`rounded-full py-1.5 ${
                  active
                    ? "bg-axiom-accent font-semibold text-white"
                    : "font-medium hover:bg-black/[0.03]"
                }`}
              >
                {d}
              </div>
            );
          })}
        </div>
      </div>

      <div className="card flex-1 p-4">
        <div className="mb-3 text-sm font-semibold">预警事件</div>
        <div className="space-y-1">
          {events.length === 0 ? (
            <div className="py-6 text-center text-xs text-axiom-muted">暂无预警</div>
          ) : (
            events.map((e) => (
              <button
                key={`${e.clueId ?? e.title}-${e.time}`}
                type="button"
                onClick={() => onOpenEvent?.(e)}
                className="flex w-full gap-3 rounded-2xl px-2 py-2.5 text-left transition hover:bg-black/[0.03]"
              >
                <div
                  className={`mt-1.5 h-2.5 w-2.5 shrink-0 rounded-full ${levelDot(
                    e.level,
                  )}`}
                />
                <div className="min-w-0">
                  <div className="text-sm font-semibold leading-snug">{e.title}</div>
                  <div className="mt-0.5 text-xs font-medium text-axiom-muted">
                    {e.time} · L{e.level}
                  </div>
                </div>
              </button>
            ))
          )}
        </div>
      </div>
    </aside>
  );
}
