import { useState } from "react";
import { useStore } from "../store/sessionStore";

interface SessionSetupProps {
  onConnectI2p: () => Promise<void>;
  onInitiateSession: (qrPayload: string) => Promise<void>;
}

export default function SessionSetup({ onConnectI2p, onInitiateSession }: SessionSetupProps) {
  const { state } = useStore();
  const [peerInput, setPeerInput] = useState("");
  const [connecting, setConnecting] = useState(false);
  const [loading, setLoading] = useState(false);
  const [tab, setTab] = useState<"show" | "connect">("show");

  const handleConnectI2p = async () => {
    setLoading(true);
    try {
      await onConnectI2p();
    } finally {
      setLoading(false);
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
            {t === "show" ? "your address" : "connect to peer"}
          </button>
        ))}
      </div>

      <div className="flex-1 overflow-y-auto flex flex-col items-center justify-center px-6 py-8 gap-6">
        {tab === "show" ? (
          <>
            {!state.i2pConnected ? (
              <div className="flex flex-col items-center gap-4 text-center">
                <p className="text-[11px] font-mono text-muted uppercase tracking-widest">
                  i2p not connected
                </p>
                <button
                  onClick={handleConnectI2p}
                  disabled={loading}
                  className="px-4 py-2 border border-border rounded text-xs font-mono text-white hover:border-white disabled:opacity-40 transition-colors uppercase tracking-wider"
                >
                  {loading ? "connecting..." : "connect to i2p"}
                </button>
                <p className="text-[10px] text-muted max-w-xs leading-relaxed">
                  requires i2pd or Java I2P running with SAM bridge enabled on{" "}
                  <span className="font-mono">{state.settings.sam_address}</span>
                </p>
              </div>
            ) : (
              <>
                {/* QR code */}
                {state.identity?.qr_svg ? (
                  <div className="qr-container p-3 border border-border rounded-lg bg-card">
                    <div
                      dangerouslySetInnerHTML={{ __html: state.identity.qr_svg }}
                      style={{ width: 200, height: 200 }}
                    />
                  </div>
                ) : (
                  <div className="w-[200px] h-[200px] border border-border rounded-lg bg-card flex items-center justify-center">
                    <span className="text-[10px] font-mono text-muted">generating...</span>
                  </div>
                )}

                {/* Address */}
                {state.identity?.b32_addr && (
                  <div className="w-full max-w-xs">
                    <p className="text-[10px] font-mono text-muted uppercase tracking-widest mb-1.5">
                      your address
                    </p>
                    <div className="bg-card border border-border rounded px-2.5 py-2">
                      <p className="font-mono text-[10px] text-secondary break-all select-text leading-relaxed">
                        {state.identity.b32_addr}
                      </p>
                    </div>
                  </div>
                )}

                <p className="text-[10px] text-muted text-center leading-relaxed max-w-xs">
                  have your peer scan this QR or paste your address. no server involved.
                </p>
              </>
            )}
          </>
        ) : (
          <div className="w-full max-w-xs flex flex-col gap-4">
            <div>
              <label className="text-[10px] font-mono text-muted uppercase tracking-widest block mb-1.5">
                peer qr payload or address
              </label>
              <textarea
                value={peerInput}
                onChange={(e) => setPeerInput(e.target.value)}
                placeholder={'{"dest":"...","k":"...","s":"..."}'}
                rows={4}
                className="w-full bg-card border border-border rounded px-3 py-2 text-[11px] font-mono text-white placeholder:text-muted focus:outline-none focus:border-secondary resize-none transition-colors"
              />
            </div>

            <button
              onClick={handleInitiate}
              disabled={!peerInput.trim() || connecting || !state.i2pConnected}
              className="w-full py-2.5 border border-border rounded text-xs font-mono text-white hover:border-white disabled:opacity-40 transition-colors uppercase tracking-widest"
            >
              {connecting ? "initiating..." : "initiate session"}
            </button>

            {!state.i2pConnected && (
              <p className="text-[10px] text-muted text-center">
                connect to i2p first
              </p>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
