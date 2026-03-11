export type TtlOption = 30 | 60 | 300 | 0;

export type RouterStatus = "idle" | "bootstrapping" | "connecting" | "ready" | "error";

export interface MessageView {
  id: string;
  content: string;
  is_mine: boolean;
  timestamp: number;
  expires_at: number;
}

export interface IdentityInfo {
  b32_addr: string;
  ik_pub_hex: string;
  spk_pub_hex: string;
  connect_link: string;
}

export interface AppSettings {
  ttl_seconds: TtlOption;
}

export type AppView = "setup" | "chat";

export interface AppState {
  view: AppView;
  identity: IdentityInfo | null;
  session: { peer_dest: string } | null;
  messages: MessageView[];
  settings: AppSettings;
  routerStatus: RouterStatus;
  error: string | null;
}

export type AppAction =
  | { type: "SET_IDENTITY"; payload: IdentityInfo }
  | { type: "UPDATE_IDENTITY_ADDRESS"; payload: { b32_addr: string; connect_link: string } }
  | { type: "SET_ROUTER_STATUS"; payload: RouterStatus }
  | { type: "SESSION_ESTABLISHED"; payload: { peer_dest: string } }
  | { type: "SESSION_CLOSED" }
  | { type: "SET_MESSAGES"; payload: MessageView[] }
  | { type: "ADD_MESSAGE"; payload: MessageView }
  | { type: "SET_SETTINGS"; payload: AppSettings }
  | { type: "SET_ERROR"; payload: string | null }
  | { type: "PANIC_WIPE" };
