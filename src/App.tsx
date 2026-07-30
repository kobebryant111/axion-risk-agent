import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Sidebar, type NavKey } from "./components/Sidebar";
import { demoClues, demoDashboard } from "./data/demo";
import type { DashboardSnapshot, RiskClue } from "./data/types";
import { Overview } from "./pages/Overview";
import { Placeholder } from "./pages/Placeholder";

async function loadDashboard(): Promise<DashboardSnapshot> {
  try {
    return await invoke<DashboardSnapshot>("get_dashboard_snapshot");
  } catch {
    return demoDashboard;
  }
}

async function loadClues(): Promise<RiskClue[]> {
  try {
    return await invoke<RiskClue[]>("list_risk_clues");
  } catch {
    return demoClues;
  }
}

export default function App() {
  const [nav, setNav] = useState<NavKey>("overview");
  const [dash, setDash] = useState<DashboardSnapshot>(demoDashboard);
  const [clues, setClues] = useState<RiskClue[]>(demoClues);

  useEffect(() => {
    void (async () => {
      const [d, c] = await Promise.all([loadDashboard(), loadClues()]);
      setDash(d);
      setClues(c);
    })();
  }, []);

  return (
    <div className="box-border flex h-full gap-4 bg-axiom-bg p-4">
      <Sidebar active={nav} onChange={setNav} />
      {nav === "overview" && <Overview dash={dash} clues={clues} />}
      {nav === "whistle" && (
        <Placeholder
          title="风险吹哨"
          desc="日度/周度舆情监测、降噪过滤、四级预警与标准化风险表。下一迭代接入黑猫与规则引擎。"
        />
      )}
      {nav === "guardian" && (
        <Placeholder
          title="经营守护"
          desc="财报导入、盈利/偿债/运营/现金流四维分析与融担专项指标，输出季度经营评估报告。"
        />
      )}
      {nav === "scout" && (
        <Placeholder
          title="准入瞭望"
          desc="一票否决 / 重大违规 / 关注事项实时研判，输出通过、有条件通过或否决意见书。"
        />
      )}
      {nav === "partners" && (
        <Placeholder
          title="机构名单"
          desc="维护六类合作主体主数据：全称、简称、集团、USCC、关联方与合作状态。"
        />
      )}
      {nav === "rules" && (
        <Placeholder
          title="规则库"
          desc="四级预警规则、关键词双条件匹配、信息源可信度分级与版本管理。"
        />
      )}
      {nav === "settings" && (
        <Placeholder
          title="设置"
          desc="LLM 网关、外部数据源凭证、跑批调度与本地审计日志配置。"
        />
      )}
    </div>
  );
}
