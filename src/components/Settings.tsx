import { useState } from "react";
import { useStore } from "../store/sessionStore";
import type { TtlOption } from "../types";

interface SettingsProps {
  onClose: () => void;
  onSave: (settings: { ttl_seconds: TtlOption; sam_address: string }) => Promise<void>;
}

const TTL_OPTIONS: { value: TtlOption; label: string }[] = [
  { value: 30, label: "30 seconds" },
  { value: 60, label: "1 minute" },
  { value: 300, label: "5 minutes" },
  { value: 0, label: "session only" },
];

export default function Settings({ onClose, onSave }: SettingsProps) {
  const { state } = useStore();
  const [ttl, setTtl] = useState<TtlOption>(state.settings.ttl_seconds);
  const [samAddr, setSamAddr] = useState(state.settings.sam_address);
  const [saving, setSaving] = useState(false);

  const handleSave = async () => {
    setSaving(true);
    try {
      await onSave({ ttl_seconds: ttl, sam_address: samAddr });
      onClose();
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="absolute inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-end">
      <div className="w-full bg-surface border-t border-border rounded-t-xl p-5 flex flex-col gap-5">
        {/* Header */}
        <div className="flex items-center justify-between">
          <span className="text-[11px] font-mono uppercase tracking-widest text-muted">
            settings
          </span>
          <button
            onClick={onClose}
            className="text-muted hover:text-white transition-colors"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>

        {/* TTL */}
        <div>
          <p className="text-[10px] font-mono text-muted uppercase tracking-widest mb-3">
            message ttl
          </p>
          <div className="grid grid-cols-2 gap-2">
            {TTL_OPTIONS.map((opt) => (
              <button
                key={opt.value}
                onClick={() => setTtl(opt.value)}
                className={`py-2 px-3 rounded border text-[11px] font-mono transition-colors ${
                  ttl === opt.value
                    ? "border-white text-white"
                    : "border-border text-muted hover:border-secondary hover:text-secondary"
                }`}
              >
                {opt.label}
              </button>
            ))}
          </div>
        </div>

        {/* SAM bridge */}
        <div>
          <label className="text-[10px] font-mono text-muted uppercase tracking-widest block mb-1.5">
            sam bridge address
          </label>
          <input
            type="text"
            value={samAddr}
            onChange={(e) => setSamAddr(e.target.value)}
            className="w-full bg-card border border-border rounded px-3 py-2 text-sm font-mono text-white placeholder:text-muted focus:outline-none focus:border-secondary transition-colors"
            placeholder="127.0.0.1:7656"
          />
          <p className="text-[10px] text-muted mt-1.5 leading-relaxed">
            i2pd SAM bridge. default port 7656.
          </p>
        </div>

        {/* Info */}
        <div className="border border-border-subtle rounded p-3 flex flex-col gap-1.5">
          <p className="text-[10px] font-mono text-muted uppercase tracking-wider">security model</p>
          <p className="text-[10px] text-muted leading-relaxed">
            all messages stored in RAM only. x3dh key exchange + double ratchet e2ee.
            traffic routed over i2p — no direct ip exchange.
          </p>
        </div>

        <button
          onClick={handleSave}
          disabled={saving}
          className="w-full py-2.5 border border-white rounded text-xs font-mono text-white hover:bg-white hover:text-black disabled:opacity-40 transition-colors uppercase tracking-widest"
        >
          {saving ? "saving..." : "save"}
        </button>
      </div>
    </div>
  );
}
