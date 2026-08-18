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
        <div className="flex items-end justify-between gap-4">
          <div>
            <div className="text-xs font-semibold text-axiom-accent">总览</div>
            <h1 className="mt-1 text-2xl font-bold tracking-tight">风险监测看板</h1>
          </div>
          <div className="text-sm text-axiom-muted">
            {dash.monitorDate}
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
