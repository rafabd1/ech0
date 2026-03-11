export type TtlOption = 30 | 60 | 300 | 0;

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
  qr_svg: string;
}

export interface PeerInfo {
  dest: string;
  k: string;
  s: string;
}

export interface ConnectionStatus {
  connected: boolean;
  peer_dest: string | null;
  sam_reachable: boolean;
}

export interface AppSettings {
  ttl_seconds: TtlOption;
  sam_address: string;
}

export type AppView = "setup" | "chat" | "settings";

export interface AppState {
  view: AppView;
  identity: IdentityInfo | null;
  session: { peer_dest: string } | null;
  messages: MessageView[];
  settings: AppSettings;
  i2pConnected: boolean;
  samReachable: boolean;
  error: string | null;
}

export type AppAction =
  | { type: "SET_IDENTITY"; payload: IdentityInfo }
  | { type: "UPDATE_IDENTITY_QR"; payload: { b32_addr: string; qr_svg: string } }
  | { type: "SET_I2P_STATUS"; payload: { connected: boolean; reachable: boolean } }
  | { type: "SESSION_ESTABLISHED"; payload: { peer_dest: string } }
  | { type: "SESSION_CLOSED" }
  | { type: "SET_MESSAGES"; payload: MessageView[] }
  | { type: "ADD_MESSAGE"; payload: MessageView }
  | { type: "SET_VIEW"; payload: AppView }
  | { type: "SET_SETTINGS"; payload: AppSettings }
  | { type: "SET_ERROR"; payload: string | null }
  | { type: "PANIC_WIPE" };
