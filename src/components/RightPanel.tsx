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
}: {
  events: EventItem[];
  monitorDate: string;
}) {
  const day = Number(monitorDate.split("-")[2] ?? "27");
  const days = Array.from({ length: 31 }, (_, i) => i + 1);

  return (
    <aside className="flex w-[280px] shrink-0 flex-col gap-4">
      <div className="card p-4">
        <div className="mb-3 flex items-center justify-between">
          <div className="text-sm font-semibold">2026 年 7 月</div>
          <div className="text-xs text-axiom-muted">监测日历</div>
        </div>
        <div className="grid grid-cols-7 gap-1 text-center text-[11px] text-axiom-muted">
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
                    : "hover:bg-black/[0.03]"
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
        <div className="space-y-3">
          {events.map((e) => (
            <div key={`${e.title}-${e.time}`} className="flex gap-3">
              <div
                className={`mt-1.5 h-2.5 w-2.5 shrink-0 rounded-full ${levelDot(
                  e.level,
                )}`}
              />
              <div>
                <div className="text-sm font-medium leading-snug">{e.title}</div>
                <div className="mt-0.5 text-[11px] text-axiom-muted">
                  {e.time} · L{e.level}
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>
    </aside>
  );
}
