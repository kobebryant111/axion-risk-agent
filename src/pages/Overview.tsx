import { useState } from "react";
import { updateClue } from "../api";
import { ClueDetailModal } from "../components/ClueDetailModal";
import { ClueList } from "../components/ClueList";
import { HealthGauge } from "../components/HealthGauge";
import { RightPanel } from "../components/RightPanel";
import { TrendChart } from "../components/TrendChart";
import { TypeCards } from "../components/TypeCards";
import type { DashboardSnapshot, EventItem, RiskClue } from "../data/types";

type Props = {
  dash: DashboardSnapshot;
  clues: RiskClue[];
  onCluesChanged: (clues: RiskClue[]) => void;
  onRefreshDash?: () => Promise<void>;
  onViewAllClues?: () => void;
};

export function Overview({
  dash,
  clues,
  onCluesChanged,
  onRefreshDash,
  onViewAllClues,
}: Props) {
  const [selected, setSelected] = useState<RiskClue | null>(null);

  function openClue(clue: RiskClue) {
    setSelected(clue);
  }

  function openEvent(event: EventItem) {
    if (event.clueId) {
      const byId = clues.find((c) => c.id === event.clueId);
      if (byId) {
        setSelected(byId);
        return;
      }
    }
    const hit = clues.find((c) => c.title === event.title);
    if (hit) setSelected(hit);
  }

  function applyUpdated(clue: RiskClue, updated: RiskClue) {
    const next = { ...clue, ...updated };
    onCluesChanged(clues.map((c) => (c.id === updated.id ? next : c)));
    setSelected(next);
    void onRefreshDash?.();
  }

  async function changeLevel(clue: RiskClue, level: number) {
    const updated = await updateClue({
      clueId: clue.id,
      level,
      actor: "风控同学",
    });
    applyUpdated(clue, updated);
  }

  async function closeClue(clue: RiskClue) {
    const updated = await updateClue({
      clueId: clue.id,
      status: "已关闭",
      actor: "风控同学",
    });
    applyUpdated(clue, updated);
  }

  async function startHandle(clue: RiskClue) {
    const updated = await updateClue({
      clueId: clue.id,
      status: "跟踪中",
      actor: "风控同学",
    });
    applyUpdated(clue, updated);
  }

  return (
    <div className="flex min-h-0 flex-1 gap-4 overflow-hidden">
      <div className="flex min-w-0 flex-1 flex-col gap-4 overflow-y-auto pr-1">
        <div className="flex items-end justify-between gap-4">
          <div>
            <div className="text-xs font-semibold text-axiom-accent">总览</div>
            <h1 className="mt-1 text-2xl font-bold tracking-tight">风险监测看板</h1>
          </div>
          <div className="text-sm font-medium text-axiom-muted">
            {dash.monitorDate}
          </div>
        </div>

        <div className="grid grid-cols-4 gap-3">
          {[
            { label: "昨日有效", value: dash.kpiEffective, tone: "text-axiom-text" },
            { label: "1 级致命", value: dash.kpiLevel1, tone: "text-red-600" },
            { label: "2 级重大", value: dash.kpiLevel2, tone: "text-orange-600" },
            { label: "待复核", value: dash.kpiPending, tone: "text-axiom-accent" },
          ].map((k) => (
            <div key={k.label} className="card px-4 py-3">
              <div className="text-sm font-medium text-axiom-muted">{k.label}</div>
              <div className={`mt-1 text-2xl font-bold tracking-tight ${k.tone}`}>{k.value}</div>
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

        <ClueList
          clues={clues}
          onOpenClue={openClue}
          onViewAll={onViewAllClues}
        />
      </div>

      <RightPanel
        events={dash.events}
        monitorDate={dash.monitorDate}
        onOpenEvent={openEvent}
      />

      {selected && (
        <ClueDetailModal
          clue={selected}
          onClose={() => setSelected(null)}
          onStartHandle={startHandle}
          onChangeLevel={changeLevel}
          onCloseClue={closeClue}
        />
      )}
    </div>
  );
}
