import { createContext, useContext, useReducer, type Dispatch } from "react";
import type { AppState, AppAction, MessageView } from "../types";

const initialState: AppState = {
  view: "setup",
  identity: null,
  session: null,
  messages: [],
  settings: { ttl_seconds: 300 },
  routerStatus: "idle",
  error: null,
};

function reducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case "SET_IDENTITY":
      return { ...state, identity: action.payload, error: null };

    case "UPDATE_IDENTITY_ADDRESS":
      if (!state.identity) return state;
      return {
        ...state,
        identity: { ...state.identity, ...action.payload },
      };

    case "SET_ROUTER_STATUS":
      return { ...state, routerStatus: action.payload };

    case "SESSION_ESTABLISHED":
      return {
        ...state,
        session: { peer_dest: action.payload.peer_dest, safety_numbers: null },
        view: "chat",
        messages: [],
        error: null,
      };

    case "SET_SAFETY_NUMBERS":
      if (!state.session) return state;
      return { ...state, session: { ...state.session, safety_numbers: action.payload } };

    case "SESSION_CLOSED":
      return { ...state, session: null, messages: [], view: "setup" };

    case "SET_MESSAGES":
      return { ...state, messages: action.payload };

    case "ADD_MESSAGE": {
      const exists = state.messages.some((m) => m.id === action.payload.id);
      if (exists) return state;
      return { ...state, messages: [...state.messages, action.payload] };
    }

    case "SET_SETTINGS":
      return { ...state, settings: action.payload };

    case "SET_ERROR":
      return { ...state, error: action.payload };

    case "PANIC_WIPE":
      return {
        ...initialState,
        settings: state.settings,
        routerStatus: "connecting",
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
