import { useEffect, useState, useReducer, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { StoreContext } from "./store/sessionStore";
import type { IdentityInfo, MessageView, TtlOption } from "./types";
import Header from "./components/Header";
import ChatWindow from "./components/ChatWindow";
import MessageInput from "./components/MessageInput";
import SessionSetup from "./components/SessionSetup";
import Settings from "./components/Settings";

// -- Reducer (inline to keep App self-contained) --
import type { AppState, AppAction } from "./types";

const initial: AppState = {
  view: "setup",
  identity: null,
  session: null,
  messages: [],
  settings: { ttl_seconds: 300, sam_address: "127.0.0.1:7656" },
  i2pConnected: false,
  samReachable: false,
  error: null,
};

function reducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case "SET_IDENTITY": return { ...state, identity: action.payload, error: null };
    case "UPDATE_IDENTITY_QR":
      if (!state.identity) return state;
      return { ...state, identity: { ...state.identity, ...action.payload } };
    case "SET_I2P_STATUS":
      return { ...state, i2pConnected: action.payload.connected, samReachable: action.payload.reachable };
    case "SESSION_ESTABLISHED":
      return { ...state, session: { peer_dest: action.payload.peer_dest }, view: "chat", messages: [], error: null };
    case "SESSION_CLOSED":
      return { ...state, session: null, messages: [], view: "setup" };
    case "SET_MESSAGES": return { ...state, messages: action.payload };
    case "ADD_MESSAGE": {
      const exists = state.messages.some((m) => m.id === action.payload.id);
      if (exists) return state;
      return { ...state, messages: [...state.messages, action.payload] };
    }
    case "SET_VIEW": return { ...state, view: action.payload };
    case "SET_SETTINGS": return { ...state, settings: action.payload };
    case "SET_ERROR": return { ...state, error: action.payload };
    case "PANIC_WIPE":
      return { ...initial, settings: state.settings, identity: state.identity ? { ...state.identity, qr_svg: "" } : null };
    default: return state;
  }
}

export default function App() {
  const [state, dispatch] = useReducer(reducer, initial);
  const [showSettings, setShowSettings] = useState(false);

  // -- Bootstrap: generate identity on mount --
  useEffect(() => {
    invoke<IdentityInfo>("generate_identity")
      .then((info) => dispatch({ type: "SET_IDENTITY", payload: info }))
      .catch((e) => dispatch({ type: "SET_ERROR", payload: String(e) }));
  }, []);

  // -- Tauri event listeners --
  useEffect(() => {
    const unlisten: Array<() => void> = [];

    listen<{ b32_addr: string; qr_svg: string }>("identity_updated", (e) => {
      dispatch({ type: "UPDATE_IDENTITY_QR", payload: e.payload });
    }).then((u) => unlisten.push(u));

    listen<{ peer_dest: string }>("session_established", (e) => {
      dispatch({ type: "SESSION_ESTABLISHED", payload: e.payload });
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
    }).then((u) => unlisten.push(u));

    return () => unlisten.forEach((fn) => fn());
  }, []);

  // -- Handlers --
  const handleConnectI2p = useCallback(async () => {
    try {
      await invoke("connect_i2p");
      dispatch({ type: "SET_I2P_STATUS", payload: { connected: true, reachable: true } });
    } catch (e) {
      dispatch({ type: "SET_ERROR", payload: String(e) });
      throw e;
    }
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

  const handlePanicWipe = useCallback(async () => {
    try {
      await invoke("panic_wipe");
      // Backend emits "panic_wipe" event which dispatches PANIC_WIPE
    } catch {
      // If invoke fails, still wipe frontend state
      dispatch({ type: "PANIC_WIPE" });
    }
  }, []);

  const handleSaveSettings = useCallback(async (settings: { ttl_seconds: TtlOption; sam_address: string }) => {
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
      <div className="h-full flex flex-col bg-black relative overflow-hidden">
        <Header onSettingsToggle={() => setShowSettings((v) => !v)} onPanicWipe={handlePanicWipe} />

        {/* Error banner */}
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

        {/* Main content */}
        {state.view === "setup" || !state.session ? (
          <SessionSetup
            onConnectI2p={handleConnectI2p}
            onInitiateSession={handleInitiateSession}
          />
        ) : (
          <>
            {/* Peer info bar */}
            <div className="shrink-0 px-4 py-2 border-b border-border flex items-center justify-between">
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

            <ChatWindow />
            <MessageInput onSend={handleSendMessage} disabled={!state.session} />
          </>
        )}

        {/* Settings overlay */}
        {showSettings && (
          <Settings
            onClose={() => setShowSettings(false)}
            onSave={handleSaveSettings}
          />
        )}
      </div>
    </StoreContext.Provider>
  );
}
