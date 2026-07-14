import "./style.css";
import { ClientTransport } from "./transport";
import type { ActionRequest, CallRequest, GameStateView, JoinInfo, RoomInfo, ServerEnvelope } from "./protocol";
import { playerIndex } from "./protocol";

const root = document.querySelector<HTMLDivElement>("#app");
if (!root) throw new Error("应用根节点不存在");
const appRoot: HTMLDivElement = root;

let transport: ClientTransport | undefined;
let session: JoinInfo | undefined;
let room: RoomInfo | undefined;
let statusMessage = "服务器尚未连接";
let latestMessage = "";
let gameState: GameStateView | undefined;
let actionRequest: ActionRequest | undefined;
let callRequest: CallRequest | undefined;
let actionDeadline = 0;
let reconnectTimer: number | undefined;
let reconnectAttempts = 0;
let intentionalClose = false;

const DEFAULT_SERVER = "http://127.0.0.1:3000";

function savedServerAddress(): string {
  return localStorage.getItem("riichi.serverAddress") || DEFAULT_SERVER;
}

renderJoin();

function renderJoin(): void {
  appRoot.innerHTML = `
    <section class="shell">
      <div class="brand-mark">麻</div>
      <p class="eyebrow">RIICHI MAHJONG</p>
      <h1>和朋友打<br /><span>一局半庄</span></h1>
      <p class="intro">连接服务器，输入房间码，准备开始游戏。</p>
      <form class="join-card" id="join-form">
        <label><span>服务器地址</span><input name="server" value="${escapeHtml(savedServerAddress())}" autocomplete="url" /></label>
        <label><span>昵称</span><input name="nickname" placeholder="例如：天凤玩家" maxlength="20" required /></label>
        <label><span>房间码</span><input name="room" placeholder="留空创建新房间" maxlength="6" /></label>
        <button type="submit">进入房间 <span>→</span></button>
        <p class="status" id="status" aria-live="polite">${statusMessage}</p>
      </form>
    </section>
  `;
  document.querySelector<HTMLFormElement>("#join-form")?.addEventListener("submit", joinRoom);
}

async function joinRoom(event: SubmitEvent): Promise<void> {
  event.preventDefault();
  const form = event.currentTarget as HTMLFormElement;
  const values = new FormData(form);
  const server = String(values.get("server") || "").trim();
  const nickname = String(values.get("nickname") || "").trim();
  const requestedRoom = String(values.get("room") || "").trim().toUpperCase();
  if (!server || !nickname) return;

  try {
    localStorage.setItem("riichi.serverAddress", server);
    statusMessage = "正在连接服务器…";
    renderJoin();
    transport = new ClientTransport(server);
    const created = requestedRoom ? undefined : await transport.createRoom();
    const joined = await transport.joinRoom(created?.id ?? requestedRoom, nickname);
    session = joined;
    room = joined.room;
    renderLobby();
  } catch (error) {
    statusMessage = error instanceof Error ? error.message : "无法连接服务器";
    renderJoin();
  }
}

function renderLobby(): void {
  if (!session || !room) return;
  const ownPlayer = playerIndex(session.player);
  appRoot.innerHTML = `
    <section class="shell lobby-shell">
      <div class="lobby-topline"><span class="eyebrow">WAITING ROOM</span><button class="text-button" id="leave-button">退出</button></div>
      <h1 class="lobby-title">房间 <span>${escapeHtml(room.id)}</span></h1>
      <p class="intro">把房间码发给朋友，四人准备后即可开始。</p>
      <div class="player-list" id="player-list">${renderPlayers(ownPlayer)}</div>
      <div class="lobby-actions">
        <button id="ready-button">${room.players.find((player) => player.id[0] === ownPlayer)?.ready ? "取消准备" : "准备"}</button>
        <button class="secondary-button" id="start-button" ${room.players.length < 4 || !room.players.every((player) => player.ready) ? "disabled" : ""}>开始半庄</button>
      </div>
      <p class="status" id="status" aria-live="polite">${statusMessage}</p>
    </section>
  `;
  document.querySelector<HTMLButtonElement>("#ready-button")?.addEventListener("click", toggleReady);
  document.querySelector<HTMLButtonElement>("#start-button")?.addEventListener("click", startGame);
  document.querySelector<HTMLButtonElement>("#leave-button")?.addEventListener("click", () => {
    transport?.close();
    session = undefined;
    room = undefined;
    renderJoin();
  });
}

function renderPlayers(ownPlayer: number): string {
  return (room?.players ?? []).map((player) => `
    <div class="player-row ${player.id[0] === ownPlayer ? "own-player" : ""}">
      <span class="seat">${["东", "南", "西", "北"][player.id[0]] ?? "?"}</span>
      <span class="player-name">${escapeHtml(player.nickname)}</span>
      <span class="ready-state ${player.ready ? "is-ready" : ""}">${player.ready ? "已准备" : "等待中"}</span>
    </div>
  `).join("") || `<div class="empty-state">等待玩家加入…</div>`;
}

async function toggleReady(): Promise<void> {
  if (!transport || !session || !room) return;
  const current = room.players.find((player) => player.id[0] === playerIndex(session!.player));
  try {
    room = await transport.setReady(room.id, session.token, !current?.ready);
    statusMessage = room.players.every((player) => player.ready) ? "四人已准备，可以开始" : "准备状态已更新";
    renderLobby();
  } catch (error) {
    statusMessage = error instanceof Error ? error.message : "准备失败";
    renderLobby();
  }
}

async function startGame(): Promise<void> {
  if (!transport || !session || !room) return;
  try {
    room = await transport.startRoom(room.id, session.token);
    statusMessage = "正在建立牌局连接…";
    renderTable();
    intentionalClose = false;
    reconnectAttempts = 0;
    connectGameSocket();
  } catch (error) {
    statusMessage = error instanceof Error ? error.message : "无法开始游戏";
    renderLobby();
  }
}

function renderTable(): void {
  const ownIndex = session ? playerIndex(session.player) : 0;
  const ownPlayer = gameState?.players[ownIndex];
  const names = room?.players ?? [];
  const playerName = (index: number) => escapeHtml(names.find((p) => p.id[0] === index)?.nickname ?? ["东家", "南家", "西家", "北家"][index]);
  appRoot.innerHTML = `
    <section class="table-shell">
      <header class="table-header"><span class="eyebrow">RIICHI MAHJONG</span><span class="connection-dot">● ${escapeHtml(statusMessage)}</span><button class="text-button" id="table-leave">退出</button></header>
      <div class="mahjong-table">
        <div class="table-center"><strong>${gameState ? `${windName(gameState.wind[0])}${gameState.round}局` : "等待牌局"}</strong><span>${gameState ? `剩余 ${gameState.remaining_tiles} 张 · ${phaseName(gameState.phase)}` : "等待服务器状态…"}</span></div>
        ${renderSeat(0, playerName(0), ownIndex)}${renderSeat(1, playerName(1), ownIndex)}${renderSeat(2, playerName(2), ownIndex)}${renderSeat(3, playerName(3), ownIndex)}
        <div class="dora-row">宝牌 ${gameState?.dora?.map(tileTypeLabel).join(" ") || "—"}</div>
      </div>
      <div class="game-grid">
        <section class="hand-panel"><div class="panel-heading"><span>我的手牌</span><span>${ownPlayer?.points?.toLocaleString() ?? "—"} 点</span></div><div class="tile-row">${ownPlayer?.hand?.map((tile, index) => tileButton(tile, index)).join("") || "等待快照…"}${gameState?.drawn_tile !== null && gameState?.drawn_tile !== undefined ? `<span class="draw-gap"></span>${tileButton(gameState.drawn_tile, -1)}` : ""}</div></section>
        <section class="action-panel"><p class="status" id="game-status">${escapeHtml(latestMessage || "等待牌局快照…")}</p><div id="action-buttons">${renderActions()}</div></section>
      </div>
      <section class="discards-panel"><div class="panel-heading"><span>牌河</span><span id="countdown"></span></div>${[0, 1, 2, 3].map((index) => `<div class="discard-line"><b>${["东", "南", "西", "北"][index]} ${playerName(index)}</b><span>${gameState?.players[index]?.discards?.map(tileLabel).join(" ") || "—"}</span></div>`).join("")}</section>
    </section>
  `;
  document.querySelector<HTMLButtonElement>("#table-leave")?.addEventListener("click", leaveTable);
  document.querySelectorAll<HTMLButtonElement>("[data-action]").forEach((button) => button.addEventListener("click", () => sendAction(button.dataset.action!)));
  updateCountdown();
}

function renderSeat(index: number, name: string, ownIndex: number): string {
  const active = gameState?.current_player?.[0] === index ? " active-seat" : "";
  return `<div class="table-player player-${["bottom", "right", "top", "left"][index]}${active}">${["东", "南", "西", "北"][index]}家 · ${name}${index === ownIndex ? "（我）" : ""}<small>${gameState?.players[index]?.points?.toLocaleString() ?? "—"}</small></div>`;
}

function renderActions(): string {
  if (callRequest?.player[0] === playerIndex(session!.player)) return callRequest.options.map((option, index) => Object.entries(option.call_type).map(([kind, payload]) => `<button data-action="call:${kind}:${index}">${callName(kind)}${callTiles(payload)}</button>`).join("")).join("") + `<button class="secondary-button" data-action="call:Pass:-1">跳过</button>`;
  if (actionRequest?.player[0] !== playerIndex(session!.player)) return "";
  const buttons = actionRequest.discard_options.map((tile) => `<button data-action="discard:${tile}">打 ${tileLabel(tile)}</button>`);
  if (actionRequest.can_tsumo) buttons.unshift(`<button data-action="Tsumo">自摸</button>`);
  if (actionRequest.can_riichi) buttons.push(...actionRequest.riichi_options.map((tile) => `<button data-action="riichi:${tile}">立直打 ${tileLabel(tile)}</button>`));
  buttons.push(...actionRequest.ankan_options.map((tile) => `<button data-action="ankan:${tile}">暗杠 ${tileLabel(tile)}</button>`));
  buttons.push(...actionRequest.kakan_options.map(([index, tile]) => `<button data-action="kakan:${index}:${tile}">加杠 ${tileLabel(tile)}</button>`));
  if (actionRequest.can_kyuushu) buttons.push(`<button data-action="KyuushuKyuuhai">九种九牌</button>`);
  return buttons.join("") || `<span class="status">等待操作…</span>`;
}

function sendAction(action: string): void {
  if (!transport) return;
  try {
    if (action.startsWith("discard:")) transport.send({ TurnAction: { action: { Discard: Number(action.slice(8)) } } });
    else if (action.startsWith("riichi:")) transport.send({ TurnAction: { action: { RiichiDiscard: Number(action.slice(8)) } } });
    else if (action === "Tsumo") transport.send({ TurnAction: { action: { Tsumo: null } } });
    else if (action === "KyuushuKyuuhai") transport.send({ TurnAction: { action: { KyuushuKyuuhai: null } } });
    else if (action.startsWith("ankan:")) transport.send({ TurnAction: { action: { Ankan: Number(action.slice(6)) } } });
    else if (action.startsWith("kakan:")) {
      const [, index, tile] = action.split(":");
      transport.send({ TurnAction: { action: { Kakan: [Number(index), Number(tile)] } } });
    }
    else if (action.startsWith("call:")) {
      const [, kind, indexText] = action.split(":");
      const index = Number(indexText);
      const option = index >= 0 ? callRequest?.options[index] : undefined;
      const payload = option?.call_type[kind];
      transport.send({ CallResponse: { action: { [kind]: payload ?? null } } as never });
    }
    latestMessage = "已提交操作，等待服务器确认";
    renderTable();
  } catch (error) { statusMessage = error instanceof Error ? error.message : "操作提交失败"; renderTable(); }
}

function leaveTable(): void { intentionalClose = true; if (reconnectTimer) window.clearTimeout(reconnectTimer); transport?.close(); session = undefined; room = undefined; gameState = undefined; actionRequest = undefined; callRequest = undefined; renderJoin(); }

function connectGameSocket(): void {
  if (!transport || !session || !room) return;
  transport.connect(room.id, session.token, {
    onMessage: handleServerMessage,
    onClose: () => {
      if (intentionalClose) return;
      statusMessage = reconnectAttempts < 8 ? "连接断开，正在自动重连…" : "连接断开，请检查服务器或手动刷新";
      renderTable();
      if (reconnectAttempts < 8) {
        const delay = Math.min(1000 * 2 ** reconnectAttempts, 8000);
        reconnectAttempts += 1;
        reconnectTimer = window.setTimeout(connectGameSocket, delay);
      }
    },
    onError: (message) => { statusMessage = message; renderTable(); },
  });
  statusMessage = reconnectAttempts === 0 ? "已连接，等待牌局状态…" : "已重新连接，正在恢复状态…";
  renderTable();
}

function updateCountdown(): void {
  const countdown = document.querySelector<HTMLElement>("#countdown");
  if (!countdown || !actionDeadline) return;
  const seconds = Math.max(0, Math.ceil((actionDeadline - Date.now()) / 1000));
  countdown.textContent = `操作剩余 ${seconds}s`;
  if (seconds > 0) window.setTimeout(updateCountdown, 1000);
}

function handleServerMessage(message: ServerEnvelope): void {
  const body = message.body;
  if ("StateSnapshot" in body || "StateUpdate" in body) {
    latestMessage = "已收到玩家视角状态快照";
    gameState = (body.StateSnapshot ?? body.StateUpdate) as GameStateView;
    actionRequest = undefined;
    callRequest = undefined;
  } else if ("ActionRequired" in body) {
    latestMessage = "轮到你行动";
    actionRequest = body.ActionRequired as ActionRequest;
    callRequest = undefined;
    actionDeadline = Date.now() + 30_000;
  } else if ("CallRequired" in body) {
    latestMessage = "请响应当前鸣牌或荣和窗口";
    callRequest = body.CallRequired as CallRequest;
    actionRequest = undefined;
    actionDeadline = Date.now() + 15_000;
  } else if ("GameOver" in body) {
    latestMessage = "本局已结束";
  } else if ("Error" in body) {
    latestMessage = String(body.Error);
  } else if ("CommandRejected" in body) {
    const rejection = body.CommandRejected as { reason?: string };
    latestMessage = `操作已拒绝：${rejection.reason ?? "状态已过期"}`;
  }
  const status = document.querySelector<HTMLParagraphElement>("#game-status");
  if (status) status.textContent = latestMessage;
  if (document.querySelector(".table-shell")) renderTable();
}

function tileLabel(raw: number): string {
  const type = Math.floor(raw / 4);
  if (type < 27) return `${(type % 9) + 1}${["万", "筒", "索"][Math.floor(type / 9)]}`;
  return ["东", "南", "西", "北", "白", "发", "中"][type - 27] ?? "?";
}
function tileTypeLabel(type: [number]): string { return tileLabel(type[0] * 4); }
function tileButton(tile: number, index: number): string {
  const legal = actionRequest?.discard_options.includes(tile) ?? false;
  return `<button class="tile ${index < 0 ? "drawn-tile" : ""} ${legal ? "legal-tile" : "disabled-tile"}" ${legal ? `data-action="discard:${tile}"` : "disabled"}>${tileLabel(tile)}</button>`;
}
function callTiles(payload: unknown): string {
  if (!payload || typeof payload !== "object" || !("hand_tiles" in payload)) return "";
  const tiles = (payload as { hand_tiles: number[] }).hand_tiles;
  return tiles.length ? ` · ${tiles.map(tileLabel).join("/")}` : "";
}
function windName(type: number): string { return ["东", "南", "西", "北"][type - 27] ?? "东"; }
function phaseName(phase: string): string { return ({ DrawPhase: "摸牌", ActionPhase: "行动", ResponsePhase: "响应", ChankanResponse: "抢杠", RoundOver: "本局结束" } as Record<string, string>)[phase] ?? phase; }
function callName(kind: string): string { return ({ Ron: "荣和", Pon: "碰", Chi: "吃", Minkan: "大明杠", Pass: "跳过" } as Record<string, string>)[kind] ?? kind; }

function escapeHtml(value: string): string {
  return value.replace(/[&<>'"]/g, (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", "'": "&#39;", '"': "&quot;" })[character] ?? character);
}
