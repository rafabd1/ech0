import { useState } from "react";
import { useStore } from "../store/sessionStore";

interface SessionSetupProps {
  onInitiateSession: (payload: string) => Promise<void>;
}

export default function SessionSetup({ onInitiateSession }: SessionSetupProps) {
  const { state } = useStore();
  const [peerInput, setPeerInput] = useState("");
  const [connecting, setConnecting] = useState(false);
  const [copied, setCopied] = useState(false);
  const [tab, setTab] = useState<"show" | "connect">("show");

  const isReady = state.routerStatus === "ready";
  const isLoading =
    state.routerStatus === "bootstrapping" || state.routerStatus === "connecting";

  const connectLink = state.identity?.connect_link ?? "";

  const handleCopy = async () => {
    if (!connectLink) return;
    try {
      await navigator.clipboard.writeText(connectLink);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Fallback: select the text
    }
  };

  const handleInitiate = async () => {
    const payload = peerInput.trim();
    if (!payload) return;
    setConnecting(true);
    try {
      await onInitiateSession(payload);
    } finally {
      setConnecting(false);
    }
  };

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* Tab bar */}
      <div className="flex border-b border-border shrink-0">
        {(["show", "connect"] as const).map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            className={`flex-1 py-2.5 text-[11px] font-mono uppercase tracking-widest transition-colors ${
              tab === t
                ? "text-white border-b border-white -mb-px"
                : "text-muted hover:text-secondary"
            }`}
          >
            {t === "show" ? "your link" : "connect to peer"}
          </button>
        ))}
      </div>

      <div className="flex-1 overflow-y-auto flex flex-col items-center justify-center px-6 py-8 gap-6">
        {tab === "show" ? (
          <div className="w-full max-w-xs flex flex-col gap-5">
            {isLoading && (
              <div className="flex flex-col items-center gap-3">
                <div className="w-6 h-6 border border-secondary border-t-transparent rounded-full animate-spin" />
                <p className="text-[11px] font-mono text-muted uppercase tracking-widest">
                  {state.routerStatus === "bootstrapping"
                    ? "bootstrapping i2p..."
                    : "connecting to i2p..."}
                </p>
                <p className="text-[10px] text-muted text-center leading-relaxed max-w-xs">
                  building anonymous tunnels. this may take up to 60 seconds on first run.
                </p>
              </div>
            )}

            {state.routerStatus === "error" && (
              <p className="text-[11px] font-mono text-muted text-center">
                i2p router error — check logs
              </p>
            )}

            {isReady && connectLink && (
              <>
                <div>
                  <p className="text-[10px] font-mono text-muted uppercase tracking-widest mb-1.5">
                    your session link
                  </p>
                  <div className="bg-card border border-border rounded px-3 py-2.5">
                    <p className="font-mono text-[9px] text-secondary break-all select-text leading-relaxed">
                      {connectLink}
                    </p>
                  </div>
                </div>

                <button
                  onClick={handleCopy}
                  className="w-full py-2.5 border border-border rounded text-xs font-mono text-white hover:border-white transition-colors uppercase tracking-widest"
                >
                  {copied ? "copied" : "copy link"}
                </button>

                <p className="text-[10px] text-muted text-center leading-relaxed">
                  share this link with your peer. they paste it to initiate the session.
                  no server involved.
                </p>
              </>
            )}

            {state.routerStatus === "idle" && (
              <p className="text-[11px] font-mono text-muted text-center uppercase tracking-widest">
                starting router...
              </p>
            )}
          </div>
        ) : (
          <div className="w-full max-w-xs flex flex-col gap-4">
            <div>
              <label className="text-[10px] font-mono text-muted uppercase tracking-widest block mb-1.5">
                paste peer link
              </label>
              <textarea
                value={peerInput}
                onChange={(e) => setPeerInput(e.target.value)}
                placeholder="ech0://..."
                rows={3}
                className="w-full bg-card border border-border rounded px-3 py-2 text-[11px] font-mono text-white placeholder:text-muted focus:outline-none focus:border-secondary resize-none transition-colors"
              />
            </div>

            <button
              onClick={handleInitiate}
              disabled={!peerInput.trim() || connecting || !isReady}
              className="w-full py-2.5 border border-border rounded text-xs font-mono text-white hover:border-white disabled:opacity-40 transition-colors uppercase tracking-widest"
            >
              {connecting ? "connecting..." : "initiate session"}
            </button>

            {!isReady && (
              <p className="text-[10px] text-muted text-center">
                {isLoading ? "waiting for i2p..." : "i2p not ready"}
              </p>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
