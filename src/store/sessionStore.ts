import { createContext, useContext, useReducer, type Dispatch } from "react";
import type { AppState, AppAction, MessageView } from "../types";

const initialState: AppState = {
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
    case "SET_IDENTITY":
      return { ...state, identity: action.payload, error: null };

    case "UPDATE_IDENTITY_QR":
      if (!state.identity) return state;
      return {
        ...state,
        identity: { ...state.identity, ...action.payload },
      };

    case "SET_I2P_STATUS":
      return {
        ...state,
        i2pConnected: action.payload.connected,
        samReachable: action.payload.reachable,
      };

    case "SESSION_ESTABLISHED":
      return {
        ...state,
        session: { peer_dest: action.payload.peer_dest },
        view: "chat",
        messages: [],
        error: null,
      };

    case "SESSION_CLOSED":
      return {
        ...state,
        session: null,
        messages: [],
        view: "setup",
      };

    case "SET_MESSAGES":
      return { ...state, messages: action.payload };

    case "ADD_MESSAGE": {
      const exists = state.messages.some((m) => m.id === action.payload.id);
      if (exists) return state;
      return { ...state, messages: [...state.messages, action.payload] };
    }

    case "SET_VIEW":
      return { ...state, view: action.payload };

    case "SET_SETTINGS":
      return { ...state, settings: action.payload };

    case "SET_ERROR":
      return { ...state, error: action.payload };

    case "PANIC_WIPE":
      return {
        ...initialState,
        settings: state.settings,
        identity: state.identity
          ? { ...state.identity, qr_svg: "" }
          : null,
      };

    default:
      return state;
  }
}

export interface StoreContextValue {
  state: AppState;
  dispatch: Dispatch<AppAction>;
}

export const StoreContext = createContext<StoreContextValue>({
  state: initialState,
  dispatch: () => undefined,
});

export function useStore() {
  return useContext(StoreContext);
}

export function createStoreReducer() {
  return useReducer(reducer, initialState);
}

export type { MessageView };
