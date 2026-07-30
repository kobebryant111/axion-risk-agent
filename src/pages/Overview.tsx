import { ClueList } from "../components/ClueList";
import { HealthGauge } from "../components/HealthGauge";
import { RightPanel } from "../components/RightPanel";
import { TrendChart } from "../components/TrendChart";
import { TypeCards } from "../components/TypeCards";
import type { DashboardSnapshot, RiskClue } from "../data/types";

type Props = {
  dash: DashboardSnapshot;
  clues: RiskClue[];
};

export function Overview({ dash, clues }: Props) {
  return (
    <div className="flex min-h-0 flex-1 gap-4 overflow-hidden">
      <div className="flex min-w-0 flex-1 flex-col gap-4 overflow-y-auto pr-1">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h1 className="text-3xl font-bold tracking-tight">
              欢迎回来，{dash.userName}！
            </h1>
            <p className="mt-1 text-sm text-axiom-muted">
              监测日 {dash.monitorDate} · 昨日有效线索 {dash.kpiEffective} 条
            </p>
          </div>
          <div className="flex items-center gap-2">
            <div className="flex h-10 w-56 items-center gap-2 rounded-full bg-white px-4 text-sm text-axiom-muted shadow-soft">
              <span>⌕</span>
              <span>搜索机构 / USCC / 线索</span>
            </div>
            <button
              type="button"
              className="flex h-10 w-10 items-center justify-center rounded-full bg-white text-axiom-muted shadow-soft"
            >
              🔔
            </button>
            <div className="flex h-10 w-10 items-center justify-center rounded-full bg-axiom-soft text-sm font-bold text-axiom-accent">
              风
            </div>
          </div>
        </div>

        <div className="grid grid-cols-4 gap-3">
          {[
            { label: "昨日有效", value: dash.kpiEffective, tone: "text-axiom-text" },
            { label: "1 级致命", value: dash.kpiLevel1, tone: "text-red-500" },
            { label: "2 级重大", value: dash.kpiLevel2, tone: "text-orange-500" },
            { label: "待复核", value: dash.kpiPending, tone: "text-axiom-accent" },
          ].map((k) => (
            <div key={k.label} className="card px-4 py-3">
              <div className="text-xs text-axiom-muted">{k.label}</div>
              <div className={`mt-1 text-2xl font-bold ${k.tone}`}>{k.value}</div>
            </div>
          ))}
        </div>

        <TypeCards cards={dash.typeCards} />

        <div className="grid min-h-[240px] grid-cols-5 gap-4">
          <div className="col-span-3">
            <TrendChart points={dash.trend} />
          </div>
          <div className="col-span-2">
            <HealthGauge
              score={dash.healthScore}
              label={dash.healthRankLabel}
            />
          </div>
        </div>

        <ClueList clues={clues} />
      </div>

      <RightPanel events={dash.events} monitorDate={dash.monitorDate} />
    </div>
  );
}
