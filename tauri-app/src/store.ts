import type {
  ActionRequest,
  CallRequest,
  GameOverView,
  GameStateView,
  JoinInfo,
  RoundResultView,
  RoomInfo,
} from "./protocol";

/** 对局事件日志条目 */
export interface LogEntry {
  id: number;
  text: string;
  kind: "draw" | "discard" | "call" | "riichi" | "win" | "other";
}

/** 全局 UI 状态（main.ts 负责写入，页面渲染时读取） */
export interface UiState {
  session?: JoinInfo;
  room?: RoomInfo;
  statusMessage: string;
  latestMessage: string;
  gameState?: GameStateView;
  actionRequest?: ActionRequest;
  callRequest?: CallRequest;
  actionDeadline: number;
  /** 立直待选：点了立直按钮后等待从手牌选牌打出 */
  riichiPending: boolean;
  events: LogEntry[];
  roundResult?: { view: RoundResultView; nicknames: string[] };
  gameOver?: GameOverView;
}

export const ui: UiState = {
  statusMessage: "服务器尚未连接",
  latestMessage: "",
  events: [],
  actionDeadline: 0,
  riichiPending: false,
};

/** 离开牌桌时清空对局相关状态 */
export function resetTableState(): void {
  ui.session = undefined;
  ui.room = undefined;
  ui.gameState = undefined;
  ui.actionRequest = undefined;
  ui.callRequest = undefined;
  ui.events = [];
  ui.roundResult = undefined;
  ui.gameOver = undefined;
  ui.actionDeadline = 0;
  ui.riichiPending = false;
}
