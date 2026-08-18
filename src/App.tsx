import { useCallback, useEffect, useRef, useState } from "react";
import {
  importPartnersCsv,
  importRulesDocument,
  loadClues,
  loadDashboard,
  loadPartners,
  loadRules,
} from "./api";
import {
  BgImportToast,
  type BgImportJob,
  type BgImportKind,
} from "./components/BgImportToast";
import { Sidebar, type NavKey } from "./components/Sidebar";
import { emptyDashboard } from "./data/empty";
import type { DashboardSnapshot, Partner, RiskClue, RuleView } from "./data/types";
import { fileToTemplateCsv } from "./lib/excelTemplates";
import { formatInvokeError } from "./lib/fileImport";
import { AgentPage } from "./pages/Agent";
import { Overview } from "./pages/Overview";
import { PartnersPage } from "./pages/Partners";
import { GuardianPage } from "./pages/Guardian";
import { RulesPage } from "./pages/Rules";
import { ScoutPage } from "./pages/Scout";
import { SettingsPage } from "./pages/Settings";
import { WhistlePage } from "./pages/Whistle";

export default function App() {
  const [nav, setNav] = useState<NavKey>("overview");
  const [dash, setDash] = useState<DashboardSnapshot>(emptyDashboard());
  const [clues, setClues] = useState<RiskClue[]>([]);
  const [partners, setPartners] = useState<Partner[]>([]);
  const [rules, setRules] = useState<RuleView[]>([]);
  const [bgJob, setBgJob] = useState<BgImportJob | null>(null);
  const jobLock = useRef(false);

  const refreshAll = useCallback(async () => {
    const [d, c, p, r] = await Promise.all([
      loadDashboard(),
      loadClues(),
      loadPartners(),
      loadRules(),
    ]);
    setDash(d);
    setClues(c);
    setPartners(p);
    setRules(r);
  }, []);

  useEffect(() => {
    void refreshAll();
  }, [refreshAll]);

  const runBgImport = useCallback(
    async (kind: BgImportKind, file: File) => {
      if (jobLock.current) {
        setBgJob({
          kind,
          running: false,
          text: "",
          error: "已有导入任务在进行，请稍后再试",
        });
        return;
      }
      jobLock.current = true;
      setBgJob({
        kind,
        running: true,
        text: `已选择「${file.name}」，正在读取文件…`,
      });

      try {
        await new Promise((r) => setTimeout(r, 40));

        setBgJob({
          kind,
          running: true,
          text: `正在读取「${file.name}」…`,
        });
        await new Promise((r) => setTimeout(r, 40));

        const csv = await fileToTemplateCsv(file, kind);

        setBgJob({
          kind,
          running: true,
          text: `正在按固定模板导入「${file.name}」（可继续操作）…`,
        });
        await new Promise((r) => setTimeout(r, 40));

        if (kind === "partners") {
          const res = await importPartnersCsv(csv, true);
          setPartners(res.partners);
          setBgJob({
            kind,
            running: false,
            text: "",
            result: res.message,
          });
        } else {
          const res = await importRulesDocument(csv, true);
          setRules(res.rules);
          setBgJob({
            kind,
            running: false,
            text: "",
            result: res.message,
          });
        }
      } catch (e) {
        setBgJob({
          kind,
          running: false,
          text: "",
          error: formatInvokeError(e),
        });
      } finally {
        jobLock.current = false;
      }
    },
    [],
  );

  return (
    <div className="box-border flex h-full min-h-0 gap-4 overflow-hidden bg-axiom-bg p-4">
      <Sidebar active={nav} onChange={setNav} />
      {nav === "overview" && <Overview dash={dash} clues={clues} />}
      {nav === "whistle" && (
        <WhistlePage
          clues={clues}
          partners={partners}
          onCluesChanged={setClues}
          onRefreshAll={refreshAll}
        />
      )}
      {nav === "guardian" && <GuardianPage partners={partners} />}
      {nav === "scout" && <ScoutPage partners={partners} />}
      {nav === "partners" && (
        <PartnersPage
          partners={partners}
          onChanged={setPartners}
          onGoSettings={() => setNav("settings")}
          importRunning={bgJob?.running === true && bgJob.kind === "partners"}
          onStartExcelImport={(file) => void runBgImport("partners", file)}
        />
      )}
      {nav === "rules" && (
        <RulesPage
          rules={rules}
          onChanged={setRules}
          onGoSettings={() => setNav("settings")}
          importRunning={bgJob?.running === true && bgJob.kind === "rules"}
          onStartExcelImport={(file) => void runBgImport("rules", file)}
        />
      )}
      {nav === "agent" && <AgentPage onMutated={() => void refreshAll()} />}
      {nav === "settings" && <SettingsPage />}

      <BgImportToast
        job={bgJob}
        onDismiss={() => setBgJob(null)}
        onOpenTarget={(kind) => setNav(kind === "partners" ? "partners" : "rules")}
      />
    </div>
  );
}
