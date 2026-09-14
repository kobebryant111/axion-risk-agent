import { useEffect, useRef, useState, type MouseEvent } from "react";
import { flushSync } from "react-dom";
import {
  agentChat,
  deleteAgentConversation,
  getAgentConversation,
  getLlmConfig,
  listAgentConversations,
  saveAgentConversation,
} from "../api";
import type {
  AgentChatMessage,
  AgentConversationSummary,
  AgentStoredMessage,
  LlmConfigPublic,
} from "../data/types";
import { formatInvokeError } from "../lib/fileImport";

type Msg = {
  role: "user" | "assistant";
  text: string;
  traces?: string[];
  pending?: boolean;
};

const WELCOME: Msg = {
  role: "assistant",
  text: "我是智鉴风控官 Agent。配置好 LLM 后，可以问风险线索、改规则、触发跑批。",
};

const HINTS = [
  "现在有哪些 1/2 级风险？",
  "汇总一下合作机构名单",
  "规则库一共多少条？停用综合融资成本超24%那条",
  "帮我跑一次风险吹哨",
];

function toStored(messages: Msg[]): AgentStoredMessage[] {
  return messages
    .filter((m) => !m.pending)
    .filter((m) => !(m.role === "assistant" && m.text === WELCOME.text && !m.traces?.length))
    .map((m) => ({
      role: m.role,
      content: m.text,
      traces: m.traces?.length ? m.traces : undefined,
    }));
}

function fromStored(messages: AgentStoredMessage[]): Msg[] {
  if (messages.length === 0) return [WELCOME];
  return messages.map((m) => ({
    role: m.role === "user" ? "user" : "assistant",
    text: m.content,
    traces: m.traces,
  }));
}

export function AgentPage({ onMutated }: { onMutated?: () => void }) {
  const [llm, setLlm] = useState<LlmConfigPublic | null>(null);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [chat, setChat] = useState<Msg[]>([WELCOME]);
  const [conversationId, setConversationId] = useState<string | null>(null);
  const [history, setHistory] = useState<AgentConversationSummary[]>([]);
  const [histErr, setHistErr] = useState<string | null>(null);
  const bottomRef = useRef<HTMLDivElement>(null);
  const savingRef = useRef(false);

  async function refreshHistory() {
    try {
      setHistory(await listAgentConversations(80));
      setHistErr(null);
    } catch (e) {
      setHistErr(formatInvokeError(e));
    }
  }

  useEffect(() => {
    void getLlmConfig().then(setLlm);
    void refreshHistory();
  }, []);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [chat]);

  async function persist(messages: Msg[], id: string | null) {
    const stored = toStored(messages);
    if (stored.length === 0) return id;
    if (savingRef.current) return id;
    savingRef.current = true;
    try {
      const saved = await saveAgentConversation({
        id: id ?? undefined,
        messages: stored,
      });
      if (!saved) return id;
      setConversationId(saved.id);
      await refreshHistory();
      return saved.id;
    } catch (e) {
      setHistErr(formatInvokeError(e));
      return id;
    } finally {
      savingRef.current = false;
    }
  }

  function newChat() {
    setConversationId(null);
    setChat([WELCOME]);
    setInput("");
    setHistErr(null);
  }

  async function openConversation(id: string) {
    if (busy) return;
    try {
      const full = await getAgentConversation(id);
      if (!full) {
        setHistErr("对话不存在或已删除");
        await refreshHistory();
        return;
      }
      setConversationId(full.id);
      setChat(fromStored(full.messages));
      setHistErr(null);
    } catch (e) {
      setHistErr(formatInvokeError(e));
    }
  }

  async function removeConversation(id: string, e: MouseEvent) {
    e.stopPropagation();
    if (busy) return;
    try {
      await deleteAgentConversation(id);
      if (conversationId === id) newChat();
      await refreshHistory();
    } catch (err) {
      setHistErr(formatInvokeError(err));
    }
  }

  async function send(text?: string) {
    const content = (text ?? input).trim();
    if (!content || busy) return;

    const withUser: Msg[] = [
      ...chat.filter((m) => !m.pending),
      { role: "user", text: content },
    ];
    const thinking: Msg = {
      role: "assistant",
      text: "正在思考…",
      pending: true,
    };

    // 先强制画出用户消息与思考中，再调后端（避免同步命令卡死导致两者一起弹出）
    flushSync(() => {
      setInput("");
      setChat([...withUser, thinking]);
      setBusy(true);
    });
    await new Promise<void>((r) => requestAnimationFrame(() => r()));

    try {
      const historyMsgs: AgentChatMessage[] = withUser
        .filter((m) => m.role === "user" || m.role === "assistant")
        .filter((m) => !(m.role === "assistant" && m.text === WELCOME.text))
        .slice(-8)
        .map((m) => ({ role: m.role, content: m.text }));
      const res = await agentChat(content, {
        scope: "general",
        history: historyMsgs,
      });
      const next: Msg[] = [
        ...withUser,
        {
          role: "assistant",
          text: res.reply,
          traces: res.toolTraces,
        },
      ];
      setChat(next);
      await persist(next, conversationId);
      if (res.mutated) onMutated?.();
      setLlm(await getLlmConfig());
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="flex min-h-0 min-w-0 flex-1 gap-4 overflow-hidden">
      <aside className="card flex w-[240px] shrink-0 flex-col overflow-hidden">
        <div className="flex items-center justify-between gap-2 border-b border-black/[0.04] px-3 py-3">
          <div className="text-sm font-bold">历史对话</div>
          <button
            type="button"
            disabled={busy}
            onClick={newChat}
            className="rounded-full bg-axiom-accent px-2.5 py-1 text-[11px] font-semibold text-white disabled:opacity-50"
          >
            新对话
          </button>
        </div>
        <div className="min-h-0 flex-1 space-y-1 overflow-y-auto p-2">
          {histErr && (
            <div className="rounded-xl bg-red-50 px-2.5 py-2 text-[11px] font-semibold text-red-700">
              {histErr}
            </div>
          )}
          {history.length === 0 ? (
            <div className="px-2 py-8 text-center text-xs text-axiom-muted">
              暂无历史。发送消息后会自动保存。
            </div>
          ) : (
            history.map((h) => {
              const active = h.id === conversationId;
              return (
                <div
                  key={h.id}
                  className={`group flex items-start gap-1 rounded-2xl px-2.5 py-2 ${
                    active ? "bg-axiom-soft" : "hover:bg-black/[0.03]"
                  }`}
                >
                  <button
                    type="button"
                    disabled={busy}
                    onClick={() => void openConversation(h.id)}
                    className="min-w-0 flex-1 text-left disabled:opacity-50"
                  >
                    <div className="truncate text-sm font-semibold text-axiom-text">
                      {h.title}
                    </div>
                    <div className="mt-0.5 text-[11px] font-medium text-axiom-muted">
                      {h.updatedAt} · {h.messageCount} 条
                    </div>
                  </button>
                  <button
                    type="button"
                    title="删除"
                    disabled={busy}
                    onClick={(e) => void removeConversation(h.id, e)}
                    className="shrink-0 rounded-lg px-1.5 py-0.5 text-[11px] font-semibold text-axiom-muted opacity-0 hover:bg-red-50 hover:text-red-600 group-hover:opacity-100 disabled:opacity-30"
                  >
                    删
                  </button>
                </div>
              );
            })
          )}
        </div>
      </aside>

      <div className="flex min-w-0 flex-1 flex-col gap-4 overflow-hidden">
        <header className="card shrink-0 p-6">
          <div className="text-xs font-semibold text-axiom-accent">问鉴控</div>
          <h1 className="mt-1 text-2xl font-bold tracking-tight">AI Agent 对话</h1>
          <p className="mt-2 text-sm text-axiom-muted">
            {llm?.ready
              ? `已连接 · ${llm.model} · ${llm.baseUrl}`
              : "LLM 未就绪：请先到「设置」填写 API Key 并启用"}
            {" · "}对话会自动保存到本机
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
                      : m.pending
                        ? "bg-black/[0.04] text-axiom-muted"
                        : "bg-black/[0.04] text-axiom-text"
                  }`}
                >
                  {m.pending ? (
                    <span className="inline-flex items-center gap-2">
                      <span className="inline-block h-3.5 w-3.5 animate-spin rounded-full border-2 border-axiom-soft border-t-axiom-accent" />
                      {m.text}
                    </span>
                  ) : (
                    m.text
                  )}
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
      </div>
    </main>
  );
}
