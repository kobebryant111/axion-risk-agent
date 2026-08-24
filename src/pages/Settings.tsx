import { useEffect, useState } from "react";
import {
  getBaiduSearchSettings,
  getEnterpriseMcpSettings,
  getLlmConfig,
  saveBaiduSearchSettings,
  saveEnterpriseMcpSettings,
  saveLlmConfig,
  testBaiduSearch,
  testEnterpriseMcp,
  testLlmConnection,
} from "../api";
import type {
  EnterpriseMcpSettings,
  LlmConfigPublic,
  BaiduSearchSettings,
} from "../data/types";

export function SettingsPage() {
  const [cfg, setCfg] = useState<LlmConfigPublic | null>(null);
  const [baseUrl, setBaseUrl] = useState("https://api.openai.com/v1");
  const [model, setModel] = useState("gpt-4o-mini");
  const [apiKey, setApiKey] = useState("");
  const [enabled, setEnabled] = useState(false);
  const [ent, setEnt] = useState<EnterpriseMcpSettings | null>(null);
  const [qccEnabled, setQccEnabled] = useState(false);
  const [qccKey, setQccKey] = useState("");
  const [tycEnabled, setTycEnabled] = useState(false);
  const [tycKey, setTycKey] = useState("");
  const [search, setSearch] = useState<BaiduSearchSettings | null>(null);
  const [searchEnabled, setSearchEnabled] = useState(false);
  const [searchKey, setSearchKey] = useState("");
  const [searchInBatch, setSearchInBatch] = useState(true);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  useEffect(() => {
    void (async () => {
      try {
        const c = await getLlmConfig();
        setCfg(c);
        setBaseUrl(c.baseUrl || "https://api.openai.com/v1");
        setModel(c.model || "gpt-4o-mini");
        setEnabled(!!c.enabled);
        setApiKey(c.hasApiKey ? "********" : "");
      } catch (e) {
        setMsg(e instanceof Error ? e.message : String(e));
      }
      try {
        const e = await getEnterpriseMcpSettings();
        setEnt(e);
        setQccEnabled(!!e.qccEnabled);
        setQccKey(e.qccHasApiKey ? "********" : "");
        setTycEnabled(!!e.tycEnabled);
        setTycKey(e.tycHasApiKey ? "********" : "");
      } catch (e) {
        setMsg(e instanceof Error ? e.message : String(e));
      }
      try {
        const t = await getBaiduSearchSettings();
        setSearch(t);
        setSearchEnabled(!!t.enabled);
        setSearchKey(t.hasApiKey ? "********" : "");
        setSearchInBatch(t.useInBatch !== false);
      } catch (e) {
        setMsg(e instanceof Error ? e.message : String(e));
      }
    })();
  }, []);

  async function save() {
    setBusy(true);
    setMsg(null);
    try {
      const next = await saveLlmConfig({
        baseUrl,
        model,
        enabled,
        apiKey: apiKey === "********" ? undefined : apiKey,
      });
      setCfg(next);
      setApiKey(next.hasApiKey ? "********" : "");
      const e = await saveEnterpriseMcpSettings({
        qccEnabled,
        tycEnabled,
        qccApiKey: qccKey === "********" ? undefined : qccKey,
        tycApiKey: tycKey === "********" ? undefined : tycKey,
      });
      setEnt(e);
      setQccKey(e.qccHasApiKey ? "********" : "");
      setTycKey(e.tycHasApiKey ? "********" : "");
      const t = await saveBaiduSearchSettings({
        enabled: searchEnabled,
        useInBatch: searchInBatch,
        apiKey: searchKey === "********" ? undefined : searchKey,
      });
      setSearch(t);
      setSearchKey(t.hasApiKey ? "********" : "");
      setMsg(
        [
          next.ready ? "LLM 就绪" : "LLM 未就绪",
          t.ready ? "百度搜索就绪" : null,
          e.qccReady || e.tycReady ? "企业 MCP 已配置" : null,
        ]
          .filter(Boolean)
          .join(" · ") || "已保存",
      );
    } catch (e) {
      setMsg(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  async function test() {
    setBusy(true);
    setMsg(null);
    try {
      await saveLlmConfig({
        baseUrl,
        model,
        enabled: true,
        apiKey: apiKey === "********" ? undefined : apiKey,
      });
      setEnabled(true);
      const res = await testLlmConnection();
      setMsg(res.message);
      setCfg(await getLlmConfig());
    } catch (e) {
      setMsg(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  async function testMcp(provider: "qcc" | "tyc") {
    setBusy(true);
    setMsg(null);
    try {
      await saveEnterpriseMcpSettings({
        qccEnabled: provider === "qcc" ? true : qccEnabled,
        tycEnabled: provider === "tyc" ? true : tycEnabled,
        qccApiKey: qccKey === "********" ? undefined : qccKey,
        tycApiKey: tycKey === "********" ? undefined : tycKey,
      });
      if (provider === "qcc") setQccEnabled(true);
      if (provider === "tyc") setTycEnabled(true);
      const res = await testEnterpriseMcp(provider);
      setMsg(res.message);
      const e = await getEnterpriseMcpSettings();
      setEnt(e);
      setQccEnabled(e.qccEnabled);
      setTycEnabled(e.tycEnabled);
      setQccKey(e.qccHasApiKey ? "********" : "");
      setTycKey(e.tycHasApiKey ? "********" : "");
    } catch (e) {
      setMsg(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  async function testSearchConn() {
    setBusy(true);
    setMsg(null);
    try {
      await saveBaiduSearchSettings({
        enabled: true,
        useInBatch: searchInBatch,
        apiKey: searchKey === "********" ? undefined : searchKey,
      });
      setSearchEnabled(true);
      const res = await testBaiduSearch();
      setMsg(res.message);
      const t = await getBaiduSearchSettings();
      setSearch(t);
      setSearchEnabled(t.enabled);
      setSearchInBatch(t.useInBatch);
      setSearchKey(t.hasApiKey ? "********" : "");
    } catch (e) {
      setMsg(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="flex min-w-0 flex-1 flex-col gap-4 overflow-auto">
      <header className="card flex items-center justify-between gap-4 p-6">
        <h1 className="text-2xl font-bold tracking-tight">设置</h1>
        <div className="text-xs text-axiom-muted">
          {cfg?.ready ? "LLM 就绪" : "LLM 未就绪"}
          {cfg?.hasApiKey ? " · Key 已配置" : ""}
          {search?.ready ? " · 百度搜索就绪" : ""}
          {ent?.qccReady ? " · 企查查就绪" : ""}
          {ent?.tycReady ? " · 天眼查就绪" : ""}
        </div>
      </header>

      <section className="card space-y-4 p-6">
        <div className="text-base font-semibold">LLM</div>
        <div className="flex items-center justify-between gap-3">
          <div className="text-sm font-semibold">启用</div>
          <button
            type="button"
            onClick={() => setEnabled((v) => !v)}
            className={`rounded-full px-4 py-2 text-sm font-semibold ${
              enabled ? "bg-emerald-100 text-emerald-700" : "bg-slate-100 text-slate-500"
            }`}
          >
            {enabled ? "已启用" : "未启用"}
          </button>
        </div>

        <label className="block">
          <div className="mb-1 text-xs font-medium text-axiom-muted">API Base URL</div>
          <input
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)}
            placeholder="https://api.openai.com/v1"
            className="w-full rounded-2xl border border-black/[0.06] px-4 py-2.5 text-sm outline-none focus:border-axiom-accent"
          />
        </label>

        <label className="block">
          <div className="mb-1 text-xs font-medium text-axiom-muted">Model</div>
          <input
            value={model}
            onChange={(e) => setModel(e.target.value)}
            placeholder="gpt-4o-mini"
            className="w-full rounded-2xl border border-black/[0.06] px-4 py-2.5 text-sm outline-none focus:border-axiom-accent"
          />
        </label>

        <label className="block">
          <div className="mb-1 text-xs font-medium text-axiom-muted">API Key</div>
          <input
            type="password"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            placeholder="sk-..."
            className="w-full rounded-2xl border border-black/[0.06] px-4 py-2.5 text-sm outline-none focus:border-axiom-accent"
          />
        </label>

        <div className="flex flex-wrap gap-2 pt-2">
          <button
            type="button"
            disabled={busy}
            onClick={() => void save()}
            className="rounded-full bg-axiom-accent px-5 py-2.5 text-sm font-semibold text-white disabled:opacity-50"
          >
            保存
          </button>
          <button
            type="button"
            disabled={busy}
            onClick={() => void test()}
            className="rounded-full bg-axiom-soft px-5 py-2.5 text-sm font-semibold text-axiom-accent disabled:opacity-50"
          >
            测试连通
          </button>
        </div>

        {msg && (
          <div className="rounded-2xl bg-black/[0.03] px-4 py-3 text-sm text-axiom-text">
            {msg}
          </div>
        )}
      </section>

      <section className="card space-y-4 p-6">
        <div>
          <div className="text-base font-semibold">百度千帆搜索</div>
          <p className="mt-1 text-xs leading-relaxed text-axiom-muted">
            接入后，风险吹哨与准入用百度索引检索国内公开网页，问鉴控可用
            baidu_web_search。单次跑批 / 问鉴控一轮最多 2 次调用。手动选日期时按网页
            page_time 过滤自然日。Key 为千帆 API Key，本地加密。获取：
            <a
              className="ml-1 text-axiom-accent underline"
              href="https://console.bce.baidu.com/qianfan/ais/console/onlineSearch"
              target="_blank"
              rel="noreferrer"
            >
              千帆·AI 搜索控制台
            </a>
            。
          </p>
        </div>

        <div className="flex items-center justify-between gap-3">
          <div className="text-sm font-semibold">启用</div>
          <button
            type="button"
            onClick={() => setSearchEnabled((v) => !v)}
            className={`rounded-full px-4 py-2 text-sm font-semibold ${
              searchEnabled
                ? "bg-emerald-100 text-emerald-700"
                : "bg-slate-100 text-slate-500"
            }`}
          >
            {searchEnabled ? "已启用" : "未启用"}
          </button>
        </div>

        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={searchInBatch}
            onChange={(e) => setSearchInBatch(e.target.checked)}
          />
          在风险吹哨跑批中启用全网搜索（最多 2 次查询）
        </label>

        <label className="block">
          <div className="mb-1 text-xs font-medium text-axiom-muted">API Key</div>
          <input
            type="password"
            value={searchKey}
            onChange={(e) => setSearchKey(e.target.value)}
            placeholder="bce-v3/..."
            className="w-full rounded-2xl border border-black/[0.06] px-4 py-2.5 text-sm outline-none focus:border-axiom-accent"
          />
        </label>

        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            disabled={busy}
            onClick={() => void save()}
            className="rounded-full bg-axiom-accent px-5 py-2.5 text-sm font-semibold text-white disabled:opacity-50"
          >
            保存百度搜索
          </button>
          <button
            type="button"
            disabled={busy}
            onClick={() => void testSearchConn()}
            className="rounded-full bg-axiom-soft px-5 py-2.5 text-sm font-semibold text-axiom-accent disabled:opacity-50"
          >
            测试连通
          </button>
        </div>
      </section>

      <section className="card space-y-4 p-6">
        <div>
          <div className="text-base font-semibold">企业数据 MCP</div>
          <p className="mt-1 text-xs leading-relaxed text-axiom-muted">
            接入企查查 / 天眼查官方 MCP 后，鉴控对话可查询工商与风险等真实数据（按对方积分/额度计费）。全网舆情请用上方百度搜索；工商司法核验请用此处。Key 本地加密存储。获取：
            <a
              className="mx-1 text-axiom-accent underline"
              href="https://agent.qcc.com/guide"
              target="_blank"
              rel="noreferrer"
            >
              企查查
            </a>
            /
            <a
              className="ml-1 text-axiom-accent underline"
              href="https://ai.tianyancha.com/guide"
              target="_blank"
              rel="noreferrer"
            >
              天眼查
            </a>
            。
          </p>
        </div>

        <div className="space-y-3 rounded-2xl bg-black/[0.02] p-4">
          <div className="flex items-center justify-between gap-3">
            <div className="text-sm font-semibold">企查查 MCP</div>
            <button
              type="button"
              onClick={() => setQccEnabled((v) => !v)}
              className={`rounded-full px-4 py-2 text-sm font-semibold ${
                qccEnabled
                  ? "bg-emerald-100 text-emerald-700"
                  : "bg-slate-100 text-slate-500"
              }`}
            >
              {qccEnabled ? "已启用" : "未启用"}
            </button>
          </div>
          <label className="block">
            <div className="mb-1 text-xs font-medium text-axiom-muted">API Key</div>
            <input
              type="password"
              value={qccKey}
              onChange={(e) => setQccKey(e.target.value)}
              placeholder="从 agent.qcc.com 获取"
              className="w-full rounded-2xl border border-black/[0.06] px-4 py-2.5 text-sm outline-none focus:border-axiom-accent"
            />
          </label>
          <button
            type="button"
            disabled={busy}
            onClick={() => void testMcp("qcc")}
            className="rounded-full bg-axiom-soft px-4 py-2 text-xs font-semibold text-axiom-accent disabled:opacity-50"
          >
            测试企查查连通
          </button>
        </div>

        <div className="space-y-3 rounded-2xl bg-black/[0.02] p-4">
          <div className="flex items-center justify-between gap-3">
            <div className="text-sm font-semibold">天眼查 MCP</div>
            <button
              type="button"
              onClick={() => setTycEnabled((v) => !v)}
              className={`rounded-full px-4 py-2 text-sm font-semibold ${
                tycEnabled
                  ? "bg-emerald-100 text-emerald-700"
                  : "bg-slate-100 text-slate-500"
              }`}
            >
              {tycEnabled ? "已启用" : "未启用"}
            </button>
          </div>
          <label className="block">
            <div className="mb-1 text-xs font-medium text-axiom-muted">API Key</div>
            <input
              type="password"
              value={tycKey}
              onChange={(e) => setTycKey(e.target.value)}
              placeholder="从天眼 AI 平台获取"
              className="w-full rounded-2xl border border-black/[0.06] px-4 py-2.5 text-sm outline-none focus:border-axiom-accent"
            />
          </label>
          <button
            type="button"
            disabled={busy}
            onClick={() => void testMcp("tyc")}
            className="rounded-full bg-axiom-soft px-4 py-2 text-xs font-semibold text-axiom-accent disabled:opacity-50"
          >
            测试天眼查连通
          </button>
        </div>

        <button
          type="button"
          disabled={busy}
          onClick={() => void save()}
          className="rounded-full bg-axiom-accent px-5 py-2.5 text-sm font-semibold text-white disabled:opacity-50"
        >
          保存企业数据配置
        </button>
      </section>
    </main>
  );
}
