import { useEffect, useRef, useState } from "react";
import { agentChat, getLlmConfig } from "../api";
import type { AgentChatMessage, LlmConfigPublic } from "../data/types";

type Msg = { role: "user" | "assistant"; text: string; traces?: string[] };

const HINTS = [
  "现在有哪些 1/2 级风险？",
  "汇总一下合作机构名单",
  "规则库一共多少条？停用综合融资成本超24%那条",
  "帮我跑一次风险吹哨",
];

export function AgentPage({ onMutated }: { onMutated?: () => void }) {
  const [llm, setLlm] = useState<LlmConfigPublic | null>(null);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [chat, setChat] = useState<Msg[]>([
    {
      role: "assistant",
      text: "我是智联鉴控 Agent。配置好 LLM 后，可以问风险线索、改规则、触发跑批。",
    },
  ]);
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    void getLlmConfig().then(setLlm);
  }, []);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [chat]);

  async function send(text?: string) {
    const content = (text ?? input).trim();
    if (!content || busy) return;
    setInput("");
    setChat((c) => [...c, { role: "user", text: content }]);
    setBusy(true);
    try {
      const history: AgentChatMessage[] = chat
        .filter((m) => m.role === "user" || m.role === "assistant")
        .slice(-8)
        .map((m) => ({ role: m.role, content: m.text }));
      const res = await agentChat(content, { scope: "general", history });
      setChat((c) => [
        ...c,
        {
          role: "assistant",
          text: res.reply,
          traces: res.toolTraces,
        },
      ]);
      if (res.mutated) onMutated?.();
      setLlm(await getLlmConfig());
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="flex min-w-0 flex-1 flex-col gap-4 overflow-hidden">
      <header className="card shrink-0 p-6">
        <div className="text-xs font-semibold text-axiom-accent">问鉴控</div>
        <h1 className="mt-1 text-2xl font-bold tracking-tight">AI Agent 对话</h1>
        <p className="mt-2 text-sm text-axiom-muted">
          {llm?.ready
            ? `已连接 · ${llm.model} · ${llm.baseUrl}`
            : "LLM 未就绪：请先到「设置」填写 API Key 并启用"}
        </p>
      </header>

      <section className="card flex min-h-0 flex-1 flex-col overflow-hidden">
        <div className="flex-1 space-y-3 overflow-auto px-5 py-4">
          {chat.map((m, i) => (
            <div key={`${m.role}-${i}`}>
              <div
                className={`max-w-[88%] rounded-2xl px-3 py-2 text-sm whitespace-pre-wrap ${
                  m.role === "user"
                    ? "ml-auto bg-axiom-accent text-white"
                    : "bg-black/[0.04] text-axiom-text"
                }`}
              >
                {m.text}
              </div>
              {m.traces && m.traces.length > 0 && (
                <div className="mt-1 max-w-[88%] space-y-0.5 text-[11px] text-axiom-muted">
                  {m.traces.map((t) => (
                    <div key={t} className="truncate">
                      🔧 {t}
                    </div>
                  ))}
                </div>
              )}
            </div>
          ))}
          <div ref={bottomRef} />
        </div>

        <div className="flex flex-wrap gap-1 border-t border-black/[0.04] px-4 py-2">
          {HINTS.map((h) => (
            <button
              key={h}
              type="button"
              disabled={busy}
              onClick={() => void send(h)}
              className="rounded-full bg-axiom-soft px-2.5 py-1 text-[11px] text-axiom-accent disabled:opacity-50"
            >
              {h}
            </button>
          ))}
        </div>

        <form
          className="flex gap-2 border-t border-black/[0.04] p-3"
          onSubmit={(e) => {
            e.preventDefault();
            void send();
          }}
        >
          <input
            value={input}
            onChange={(e) => setInput(e.target.value)}
            disabled={busy}
            placeholder={
              llm?.ready ? "问问昨天有哪些致命风险…" : "请先配置 LLM…"
            }
            className="min-w-0 flex-1 rounded-full border border-black/[0.06] px-4 py-2.5 text-sm outline-none focus:border-axiom-accent disabled:opacity-60"
          />
          <button
            type="submit"
            disabled={busy || !input.trim()}
            className="rounded-full bg-axiom-accent px-5 py-2.5 text-sm font-semibold text-white disabled:opacity-50"
          >
            {busy ? "思考中…" : "发送"}
          </button>
        </form>
      </section>
    </main>
  );
}
