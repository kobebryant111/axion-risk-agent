import type { TrendPoint } from "../data/types";

export function TrendChart({ points }: { points: TrendPoint[] }) {
  const max = Math.max(...points.map((p) => p.value), 1);
  return (
    <div className="card flex h-full flex-col p-5">
      <div className="mb-1 flex items-center justify-between">
        <div className="text-base font-semibold">近 7 日风险线索</div>
        <div className="text-xs font-medium text-axiom-ok">+12% 周环比</div>
      </div>
      <div className="mb-4 text-xs text-axiom-muted">有效线索量（降噪后）</div>
      <div className="flex flex-1 items-end gap-3 px-1 pb-1">
        {points.map((p) => {
          const h = Math.max(18, Math.round((p.value / max) * 140));
          return (
            <div key={p.day} className="flex flex-1 flex-col items-center gap-2">
              <div
                className={`w-full max-w-[28px] rounded-full ${
                  p.highlight ? "bg-axiom-accent" : "bg-[#ece7ff]"
                }`}
                style={{ height: h }}
              />
              <div className="text-[11px] text-axiom-muted">{p.day}</div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
