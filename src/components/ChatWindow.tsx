import { useEffect, useRef, useState } from "react";
import { useStore } from "../store/sessionStore";
import type { MessageView } from "../types";

function TtlBar({ expiresAt }: { expiresAt: number }) {
  const [pct, setPct] = useState(100);
  const [critical, setCritical] = useState(false);

  useEffect(() => {
    if (expiresAt === 0) return;

    const update = () => {
      const now = Date.now() / 1000;
      const remaining = expiresAt - now;
      if (remaining <= 0) { setPct(0); return; }
      // We don't know total TTL from here; use a rough 5min max for display
      const total = 300;
      const p = Math.max(0, Math.min(100, (remaining / total) * 100));
      setPct(p);
      setCritical(remaining < 10);
    };

    update();
    const id = setInterval(update, 1000);
    return () => clearInterval(id);
  }, [expiresAt]);

  if (expiresAt === 0) return null;

  return (
    <div className="h-px w-full bg-border-subtle mt-1.5 overflow-hidden rounded-full">
      <div
        className={`h-full bg-white transition-all duration-1000 ${critical ? "ttl-critical" : ""}`}
        style={{ width: `${pct}%`, opacity: critical ? 1 : 0.25 }}
      />
    </div>
  );
}

function MessageBubble({ msg }: { msg: MessageView }) {
  const [opacity, setOpacity] = useState(1);

  useEffect(() => {
    if (msg.expires_at === 0) return;
    const update = () => {
      const now = Date.now() / 1000;
      const remaining = msg.expires_at - now;
      if (remaining <= 0) { setOpacity(0.15); return; }
      if (remaining < 30) setOpacity(Math.max(0.15, remaining / 30));
      else setOpacity(1);
    };
    update();
    const id = setInterval(update, 1000);
    return () => clearInterval(id);
  }, [msg.expires_at]);

  const isMe = msg.is_mine;

  return (
    <div
      className={`flex msg-enter ${isMe ? "justify-end" : "justify-start"}`}
      style={{ opacity }}
    >
      <div
        className={`max-w-[75%] px-3 py-2 rounded-lg text-sm leading-relaxed ${
          isMe
            ? "bg-white text-black"
            : "bg-card text-white border border-border"
        }`}
      >
        <p className="break-words whitespace-pre-wrap select-text">{msg.content}</p>
        <TtlBar expiresAt={msg.expires_at} />
      </div>
    </div>
  );
}

export default function ChatWindow() {
  const { state } = useStore();
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [state.messages.length]);

  if (state.messages.length === 0) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-2">
        <div className="w-8 h-px bg-border" />
        <p className="text-[11px] font-mono text-muted uppercase tracking-widest">
          no messages
        </p>
        <div className="w-8 h-px bg-border" />
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto px-4 py-3 flex flex-col gap-2">
      {state.messages.map((msg) => (
        <MessageBubble key={msg.id} msg={msg} />
      ))}
      <div ref={bottomRef} />
    </div>
  );
}
