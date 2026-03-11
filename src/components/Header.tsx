import { useStore } from "../store/sessionStore";
import type { RouterStatus } from "../types";

interface HeaderProps {
  onSettingsToggle: () => void;
  onWipeRequest: () => void;
}

function RouterStatusDot({ status }: { status: RouterStatus }) {
  const isReady = status === "ready";
  const isPulsing = status === "bootstrapping" || status === "connecting";
  const isError = status === "error";

  const label =
    status === "ready"
      ? "i2p"
      : status === "bootstrapping"
      ? "boot"
      : status === "connecting"
      ? "sync"
      : status === "error"
      ? "err"
      : "off";

  return (
    <div className="flex items-center gap-1.5">
      <span
        className={[
          "w-1.5 h-1.5 rounded-full",
          isReady ? "bg-white" : isError ? "bg-muted" : isPulsing ? "bg-secondary animate-pulse" : "bg-muted",
        ].join(" ")}
      />
      <span className="text-[10px] font-mono text-muted uppercase tracking-wider">
        {label}
      </span>
    </div>
  );
}

export default function Header({ onSettingsToggle, onWipeRequest }: HeaderProps) {
  const { state } = useStore();

  return (
    <header className="flex items-center justify-between px-4 h-12 border-b border-border shrink-0">
      <div className="flex items-center gap-2">
        <span className="text-sm font-semibold tracking-[0.2em] text-white uppercase">
          ech0
        </span>
        {state.session && (
          <span className="text-[10px] text-muted font-mono tracking-wider uppercase">
            — encrypted
          </span>
        )}
      </div>

      <div className="flex items-center gap-3">
        <RouterStatusDot status={state.routerStatus} />

        <button
          onClick={onSettingsToggle}
          className="w-7 h-7 flex items-center justify-center rounded text-muted hover:text-white hover:bg-card transition-colors"
          title="Settings"
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
            <circle cx="12" cy="12" r="3" />
            <path d="M12 2v2m0 16v2M4.93 4.93l1.41 1.41m11.32 11.32 1.41 1.41M2 12h2m16 0h2M4.93 19.07l1.41-1.41M17.66 6.34l1.41-1.41" />
          </svg>
        </button>

        <button
          onClick={onWipeRequest}
          className="h-7 px-2.5 flex items-center gap-1.5 rounded border border-border text-muted hover:border-white hover:text-white transition-colors text-[10px] font-mono uppercase tracking-wider"
          title="Wipe all data"
        >
          <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <polyline points="3 6 5 6 21 6" />
            <path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6" />
            <path d="M10 11v6m4-6v6" />
            <path d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2" />
          </svg>
          wipe
        </button>
      </div>
    </header>
  );
}
