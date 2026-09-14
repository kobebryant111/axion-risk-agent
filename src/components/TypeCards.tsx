import type { TypeCard } from "../data/types";

const TONE_BG: Record<string, string> = {
  violet: "bg-[#ece7ff] text-[#7c5cfc]",
  blue: "bg-[#e8f0ff] text-[#3b82f6]",
  amber: "bg-[#fff4d6] text-[#d97706]",
  rose: "bg-[#ffe8ef] text-[#e11d48]",
  sky: "bg-[#e6f7ff] text-[#0284c7]",
  orange: "bg-[#ffedd5] text-[#ea580c]",
  teal: "bg-[#ccfbf1] text-[#0f766e]",
  slate: "bg-[#e2e8f0] text-[#475569]",
};

export function TypeCards({ cards }: { cards: TypeCard[] }) {
  return (
    <div className="grid grid-cols-2 gap-3 xl:grid-cols-4">
      {cards.map((card) => (
        <div key={card.key} className="card p-4">
          <div
            className={`mb-3 inline-flex h-9 w-9 items-center justify-center rounded-2xl text-sm font-bold ${
              TONE_BG[card.tone] ?? TONE_BG.violet
            }`}
          >
            {card.title.slice(0, 1)}
          </div>
          <div className="text-sm font-semibold text-axiom-text">
            {card.title}
          </div>
          <div className="mt-0.5 text-xs font-medium text-axiom-muted">
            {card.subtitle}
          </div>
          <div className="mt-3 flex items-end justify-between text-xs">
            <div>
              <div className="font-medium text-axiom-muted">昨日新增</div>
              <div className="text-lg font-bold tracking-tight">{card.yesterdayNew}</div>
            </div>
            <div className="text-right">
              <div className="font-medium text-axiom-muted">高风险</div>
              <div className="text-lg font-bold tracking-tight text-axiom-warn">
                {card.highCount}
              </div>
            </div>
          </div>
          <div className="mt-3 h-1.5 overflow-hidden rounded-full bg-black/[0.05]">
            <div
              className="h-full rounded-full bg-axiom-accent"
              style={{ width: `${card.progress}%` }}
            />
          </div>
        </div>
      ))}
    </div>
  );
}
