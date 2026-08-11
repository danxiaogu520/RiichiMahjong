import "./style.css";
import { ClientTransport } from "./transport";
import type {
  ActionRequest,
  CallRequest,
  GameOverView,
  GameStateView,
  GameEventView,
  JoinInfo,
  RoundResultView,
  RoomInfo,
  ServerEnvelope,
} from "./protocol";
import { playerIndex } from "./protocol";
import {
  UiCallbacks,
  UiState,
  LogEntry,
  nicknamesOf,
  playEventSound,
  renderJoin,
  renderLobby,
  renderTable,
  toggleSound,
} from "./views";
import { eventText } from "./format";
import { playSound } from "./sound";

const DEFAULT_SERVER = "http://127.0.0.1:3000";

let transport: ClientTransport | undefined;
let reconnectTimer: number | undefined;
let reconnectAttempts = 0;
let intentionalClose = false;
let countdownTimer: number | undefined;
let countdownGeneration = 0;

const ui: UiState = {
  statusMessage: "服务器尚未连接",
  latestMessage: "",
  events: [],
  actionDeadline: 0,
  riichiPending: false,
};

function savedServerAddress(): string {
  return localStorage.getItem("riichi.serverAddress") || DEFAULT_SERVER;
}

const callbacks: UiCallbacks = {
  joinRoom: async (form) => {
    const server = String(form.get("server") || "").trim();
    const nickname = String(form.get("nickname") || "").trim();
    const requestedRoom = String(form.get("room") || "").trim().toUpperCase();
    if (!server || !nickname) return;
    try {
      localStorage.setItem("riichi.serverAddress", server);
      ui.statusMessage = "正在连接服务器…";
      renderJoin(ui, callbacks, savedServerAddress());
      transport = new ClientTransport(server);
      const created = requestedRoom ? undefined : await transport.createRoom();
      const joined = await transport.joinRoom(created?.id ?? requestedRoom, nickname);
      ui.session = joined;
      ui.room = joined.room;
      renderLobby(ui, callbacks);
    } catch (error) {
      ui.statusMessage = error instanceof Error ? error.message : "无法连接服务器";
      renderJoin(ui, callbacks, savedServerAddress());
    }
  },
  leaveToLobby: () => {
    transport?.close();
    ui.session = undefined;
    ui.room = undefined;
    renderJoin(ui, callbacks, savedServerAddress());
  },
  toggleReady: async () => {
    if (!transport || !ui.session || !ui.room) return;
    const own = playerIndex(ui.session.player);
    const current = ui.room.players.find((player) => player.id === own);
    try {
      ui.room = await transport.setReady(ui.room.id, ui.session.token, !current?.ready);
      const humansReady = ui.room.players.filter((player) => !player.is_ai).every((player) => player.ready);
      ui.statusMessage = humansReady ? "真人玩家已准备，可以开始" : "准备状态已更新";
      renderLobby(ui, callbacks);
    } catch (error) {
      ui.statusMessage = error instanceof Error ? error.message : "准备失败";
      renderLobby(ui, callbacks);
    }
  },
  startGame: async () => {
    if (!transport || !ui.session || !ui.room) return;
    try {
      ui.room = await transport.startRoom(ui.room.id, ui.session.token);
      ui.statusMessage = "正在建立牌局连接…";
      renderTable(ui, callbacks);
      intentionalClose = false;
      reconnectAttempts = 0;
      connectGameSocket();
    } catch (error) {
      ui.statusMessage = error instanceof Error ? error.message : "无法开始游戏";
      renderLobby(ui, callbacks);
    }
  },
  setAiCount: async (delta) => {
    if (!transport || !ui.session || !ui.room || ui.room.owner !== playerIndex(ui.session.player)) return;
    const current = ui.room.players.filter((player) => player.is_ai).length;
    const target = Math.max(0, Math.min(3, current + delta));
    if (target === current) return;
    try {
      ui.room = await transport.setAiCount(ui.room.id, ui.session.token, target);
      ui.statusMessage = `AI 补位已设置为 ${target} 个`;
      renderLobby(ui, callbacks);
    } catch (error) {
      ui.statusMessage = error instanceof Error ? error.message : "AI 设置失败";
      renderLobby(ui, callbacks);
    }
  },
  sendAction: (action) => {
    if (!transport) return;
    try {
      if (action === "Riichi") {
        // 进入立直待选状态：等玩家从手牌选牌，模拟立直打 X。
        ui.riichiPending = true;
        ui.latestMessage = "已进入立直待选，请选择要打出的牌";
        renderTable(ui, callbacks);
        return;
      }
      if (action === "RiichiCancel") {
        ui.riichiPending = false;
        ui.latestMessage = "已取消立直";
        renderTable(ui, callbacks);
        return;
      }
      if (action.startsWith("discard:")) transport.send({ TurnAction: { action: { Discard: Number(action.slice(8)) } } });
      else if (action.startsWith("riichi:")) {
        transport.send({ TurnAction: { action: { RiichiDiscard: Number(action.slice(7)) } } });
        ui.riichiPending = false;
      }
      else if (action === "Tsumo") transport.send({ TurnAction: { action: { Tsumo: null } } });
      else if (action === "KyuushuKyuuhai") transport.send({ TurnAction: { action: { KyuushuKyuuhai: null } } });
      else if (action.startsWith("ankan:")) transport.send({ TurnAction: { action: { Ankan: Number(action.slice(6)) } } });
      else if (action.startsWith("kakan:")) {
        const [, index, tile] = action.split(":");
        transport.send({ TurnAction: { action: { Kakan: [Number(index), Number(tile)] } } });
      } else if (action.startsWith("call:")) {
        const [, kind, indexText] = action.split(":");
        const index = Number(indexText);
        const option = index >= 0 ? ui.callRequest?.options[index] : undefined;
        const payload = option?.call_type[kind];
        transport.send({ CallResponse: { action: { [kind]: payload ?? null } } as never });
      }
      ui.latestMessage = "已提交操作，等待服务器确认";
      renderTable(ui, callbacks);
    } catch (error) {
      ui.statusMessage = error instanceof Error ? error.message : "操作提交失败";
      renderTable(ui, callbacks);
    }
  },
  leaveTable: () => {
    intentionalClose = true;
    if (reconnectTimer) window.clearTimeout(reconnectTimer);
    transport?.close();
    resetTableState();
    renderJoin(ui, callbacks, savedServerAddress());
  },
  closeRoundResult: () => {
    ui.roundResult = undefined;
    ui.gameOver = undefined;
    if (document.querySelector(".table-shell")) renderTable(ui, callbacks);
  },
  backToHome: () => {
    intentionalClose = true;
    if (reconnectTimer) window.clearTimeout(reconnectTimer);
    transport?.close();
    resetTableState();
    renderJoin(ui, callbacks, savedServerAddress());
  },
  toggleSound: () => {
    toggleSound();
    if (document.querySelector(".table-shell")) renderTable(ui, callbacks);
  },
};

function resetTableState(): void {
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

// ─── WebSocket 连接与消息分发 ─────────────────────────────────

function connectGameSocket(): void {
  if (!transport || !ui.session || !ui.room) return;
  transport.connect(ui.room.id, ui.session.token, {
    onMessage: handleServerMessage,
    onClose: () => {
      if (intentionalClose) return;
      ui.statusMessage = reconnectAttempts < 8 ? "连接断开，正在自动重连…" : "连接断开，请检查服务器或手动刷新";
      renderTable(ui, callbacks);
      if (reconnectAttempts < 8) {
        const delay = Math.min(1000 * 2 ** reconnectAttempts, 8000);
        reconnectAttempts += 1;
        reconnectTimer = window.setTimeout(connectGameSocket, delay);
      }
    },
    onError: (message) => {
      ui.statusMessage = message;
      renderTable(ui, callbacks);
    },
  });
  ui.statusMessage = reconnectAttempts === 0 ? "已连接，等待牌局状态…" : "已重新连接，正在恢复状态…";
  renderTable(ui, callbacks);
}

function logKind(event: GameEventView): LogEntry["kind"] {
  if ("Draw" in event) return "draw";
  if ("Discard" in event) return "discard";
  if ("Call" in event) return "call";
  if ("Riichi" in event) return "riichi";
  if ("Win" in event) return "win";
  return "other";
}

function handleServerMessage(message: ServerEnvelope): void {
  const body = message.body;
  if ("StateSnapshot" in body || "StateUpdate" in body) {
    ui.latestMessage = "已收到玩家视角状态快照";
    ui.gameState = (body.StateSnapshot ?? body.StateUpdate) as GameStateView;
    ui.actionRequest = undefined;
    ui.callRequest = undefined;
    // 立直已成立（is_riichi）或快照刷新后，退出立直待选状态。
    if (ui.session && ui.gameState.players[playerIndex(ui.session.player)]?.is_riichi) {
      ui.riichiPending = false;
    }
  } else if ("ActionRequired" in body) {
    ui.latestMessage = "轮到你行动";
    ui.actionRequest = body.ActionRequired as ActionRequest;
    ui.callRequest = undefined;
    ui.riichiPending = false;
    ui.actionDeadline = Date.now() + 30_000;
  } else if ("CallRequired" in body) {
    ui.latestMessage = "请响应当前鸣牌或荣和窗口";
    ui.callRequest = body.CallRequired as CallRequest;
    ui.actionRequest = undefined;
    ui.actionDeadline = Date.now() + 15_000;
  } else if ("Event" in body) {
    const envelope = body.Event as { event_id: number; event: GameEventView };
    const names = nicknamesOf(ui.room);
    ui.events.push({ id: envelope.event_id, text: eventText(envelope.event, names), kind: logKind(envelope.event) });
    if (ui.events.length > 120) ui.events = ui.events.slice(-120);
    playEventSound(logKind(envelope.event));
  } else if ("RoundResult" in body) {
    const view = body.RoundResult as RoundResultView;
    ui.roundResult = { view, nicknames: nicknamesOf(ui.room) };
    ui.latestMessage = `本局结束：${view.win_details.length ? view.win_details.join("；") : "流局"}`;
  } else if ("GameOver" in body) {
    ui.gameOver = body.GameOver as GameOverView;
    ui.latestMessage = "半庄结束";
    ui.actionDeadline = 0;
    // 服务器在 GameOver 后会关闭连接并释放房间，无需也不应重连。
    intentionalClose = true;
    if (reconnectTimer) window.clearTimeout(reconnectTimer);
  } else if ("PlayerControllerChanged" in body) {
    const change = body.PlayerControllerChanged as { player_id: number; is_ai: boolean; ai_takeover: boolean };
    if (ui.room) {
      ui.room = {
        ...ui.room,
        players: ui.room.players.map((player) => player.id === change.player_id
          ? { ...player, is_ai: change.is_ai, ai_takeover: change.ai_takeover, connected: true }
          : player),
      };
    }
    ui.latestMessage = change.ai_takeover ? "AI 已接管断线座位" : "已恢复真人控制";
    ui.statusMessage = ui.latestMessage;
  } else if ("Error" in body) {
    ui.latestMessage = String(body.Error);
  } else if ("CommandRejected" in body) {
    const rejection = body.CommandRejected as { reason?: string };
    ui.latestMessage = `操作已拒绝：${rejection.reason ?? "状态已过期"}`;
    playSound("decline");
  }
  refreshViews();
}

function refreshViews(): void {
  const status = document.querySelector<HTMLParagraphElement>("#game-status");
  if (status) status.textContent = ui.latestMessage;
  if (document.querySelector(".table-shell")) {
    renderTable(ui, callbacks);
    startCountdown();
  } else if (document.querySelector(".lobby-shell")) {
    renderLobby(ui, callbacks);
  }
}

// ─── 操作倒计时 ───────────────────────────────────────────────

function startCountdown(): void {
  if (countdownTimer !== undefined) window.clearTimeout(countdownTimer);
  countdownGeneration += 1;
  updateCountdown(countdownGeneration);
}

function updateCountdown(generation: number): void {
  const countdown = document.querySelector<HTMLElement>("#countdown");
  if (!ui.actionDeadline) {
    if (countdown) countdown.textContent = "";
    return;
  }
  if (generation !== countdownGeneration || !countdown) return;
  const seconds = Math.max(0, Math.ceil((ui.actionDeadline - Date.now()) / 1000));
  countdown.textContent = `操作剩余 ${seconds}s`;
  if (seconds > 0) {
    countdownTimer = window.setTimeout(() => updateCountdown(generation), 1000);
  } else {
    countdownTimer = undefined;
  }
}

// ─── 启动 ─────────────────────────────────────────────────────

renderJoin(ui, callbacks, savedServerAddress());
