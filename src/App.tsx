import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { StoreContext, createStoreReducer } from "./store/sessionStore";
import type { IdentityInfo, MessageView, RouterStatus, TtlOption } from "./types";
import Header from "./components/Header";
import ChatWindow from "./components/ChatWindow";
import MessageInput from "./components/MessageInput";
import SessionSetup from "./components/SessionSetup";
import Settings from "./components/Settings";
import WipeConfirmDialog from "./components/WipeConfirmDialog";

export default function App() {
  const [state, dispatch] = createStoreReducer();
  const [showSettings, setShowSettings] = useState(false);
  const [showWipeConfirm, setShowWipeConfirm] = useState(false);

  // Sync initial state on mount
  useEffect(() => {
    // Query current router status — avoids missing events emitted before WebView registered listeners
    invoke<string>("get_router_status")
      .then((s) => dispatch({ type: "SET_ROUTER_STATUS", payload: s as RouterStatus }))
      .catch(() => undefined);

    invoke<IdentityInfo>("generate_identity")
      .then((info) => dispatch({ type: "SET_IDENTITY", payload: info }))
      .catch((e) => dispatch({ type: "SET_ERROR", payload: String(e) }));
  }, []);

  // Tauri event listeners
  useEffect(() => {
    const unlisten: Array<() => void> = [];

    listen<{ b32_addr: string; connect_link: string }>("identity_updated", (e) => {
      dispatch({ type: "UPDATE_IDENTITY_ADDRESS", payload: e.payload });
    }).then((u) => unlisten.push(u));

    listen<string>("router_status_changed", (e) => {
      dispatch({ type: "SET_ROUTER_STATUS", payload: e.payload as RouterStatus });
    }).then((u) => unlisten.push(u));

    listen<{ peer_dest: string }>("session_established", (e) => {
      dispatch({ type: "SESSION_ESTABLISHED", payload: e.payload });
      // Fetch safety numbers right after session is established
      invoke<string>("get_safety_numbers")
        .then((nums) => dispatch({ type: "SET_SAFETY_NUMBERS", payload: nums }))
        .catch(() => undefined);
    }).then((u) => unlisten.push(u));

    listen("session_closed", () => {
      dispatch({ type: "SESSION_CLOSED" });
    }).then((u) => unlisten.push(u));

    listen<MessageView>("message_received", (e) => {
      dispatch({ type: "ADD_MESSAGE", payload: e.payload });
    }).then((u) => unlisten.push(u));

    listen<MessageView[]>("messages_updated", (e) => {
      dispatch({ type: "SET_MESSAGES", payload: e.payload });
    }).then((u) => unlisten.push(u));

    listen("panic_wipe", () => {
      dispatch({ type: "PANIC_WIPE" });
      // Re-generate a fresh identity after wipe
      invoke<IdentityInfo>("generate_identity")
        .then((info) => dispatch({ type: "SET_IDENTITY", payload: info }))
        .catch(() => undefined);
    }).then((u) => unlisten.push(u));

    return () => unlisten.forEach((fn) => fn());
  }, []);

  const handleInitiateSession = useCallback(async (payload: string) => {
    try {
      await invoke("initiate_session", { peerPayload: payload });
    } catch (e) {
      dispatch({ type: "SET_ERROR", payload: String(e) });
      throw e;
    }
  }, []);

  const handleSendMessage = useCallback(async (content: string) => {
    try {
      const msg = await invoke<MessageView>("send_message", { content });
      dispatch({ type: "ADD_MESSAGE", payload: msg });
    } catch (e) {
      dispatch({ type: "SET_ERROR", payload: String(e) });
      throw e;
    }
  }, []);

  const handleWipeConfirm = useCallback(async () => {
    setShowWipeConfirm(false);
    try {
      await invoke("panic_wipe");
    } catch {
      dispatch({ type: "PANIC_WIPE" });
    }
  }, []);

  const handleSaveSettings = useCallback(async (settings: { ttl_seconds: TtlOption }) => {
    try {
      await invoke("update_settings", { settings });
      dispatch({ type: "SET_SETTINGS", payload: settings });
    } catch (e) {
      dispatch({ type: "SET_ERROR", payload: String(e) });
      throw e;
    }
  }, []);

  return (
    <StoreContext.Provider value={{ state, dispatch }}>
      <div className="h-full flex flex-col bg-black relative overflow-hidden safe-top safe-bottom">
        <Header
          onSettingsToggle={() => setShowSettings((v) => !v)}
          onWipeRequest={() => setShowWipeConfirm(true)}
        />

        {state.error && (
          <div className="shrink-0 px-4 py-2 bg-card border-b border-border flex items-center justify-between gap-3">
            <p className="text-[11px] font-mono text-secondary truncate">{state.error}</p>
            <button
              onClick={() => dispatch({ type: "SET_ERROR", payload: null })}
              className="text-muted hover:text-white shrink-0 transition-colors"
            >
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
              </svg>
            </button>
          </div>
        )}

        {state.view === "setup" || !state.session ? (
          <SessionSetup onInitiateSession={handleInitiateSession} />
        ) : (
          <>
            <div className="shrink-0 px-4 py-2 border-b border-border">
              <div className="flex items-center justify-between">
                <p className="text-[10px] font-mono text-muted truncate max-w-[80%]">
                  {state.session.peer_dest}
                </p>
                <button
                  onClick={() => invoke("close_session").then(() => dispatch({ type: "SESSION_CLOSED" }))}
                  className="text-[10px] font-mono text-muted hover:text-white transition-colors uppercase tracking-wider"
                >
                  end
                </button>
              </div>
              {state.session.safety_numbers && (
                <p className="text-[9px] font-mono text-muted mt-0.5 select-all" title="Compare this value with your peer to verify the session">
                  verify: {state.session.safety_numbers}
                </p>
              )}
            </div>
            <ChatWindow />
            <MessageInput onSend={handleSendMessage} disabled={!state.session} />
          </>
        )}

        {showSettings && (
          <Settings
            onClose={() => setShowSettings(false)}
            onSave={handleSaveSettings}
          />
        )}

        {showWipeConfirm && (
          <WipeConfirmDialog
            onConfirm={handleWipeConfirm}
            onCancel={() => setShowWipeConfirm(false)}
          />
        )}
      </div>
    </StoreContext.Provider>
  );
}
