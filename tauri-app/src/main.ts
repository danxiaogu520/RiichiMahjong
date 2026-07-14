import "./style.css";
import { ClientTransport } from "./transport";
import type { JoinInfo, RoomInfo, ServerEnvelope } from "./protocol";
import { playerIndex } from "./protocol";

const root = document.querySelector<HTMLDivElement>("#app");
if (!root) throw new Error("应用根节点不存在");
const appRoot: HTMLDivElement = root;

let transport: ClientTransport | undefined;
let session: JoinInfo | undefined;
let room: RoomInfo | undefined;
let statusMessage = "服务器尚未连接";
let latestMessage = "";

renderJoin();

function renderJoin(): void {
  appRoot.innerHTML = `
    <section class="shell">
      <div class="brand-mark">麻</div>
      <p class="eyebrow">RIICHI MAHJONG</p>
      <h1>和朋友打<br /><span>一局半庄</span></h1>
      <p class="intro">连接服务器，输入房间码，准备开始游戏。</p>
      <form class="join-card" id="join-form">
        <label><span>服务器地址</span><input name="server" value="http://127.0.0.1:3000" autocomplete="url" /></label>
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
    transport.connect(room.id, session.token, {
      onMessage: handleServerMessage,
      onClose: () => {
        statusMessage = "连接已断开";
        renderTable();
      },
      onError: (message) => {
        statusMessage = message;
        renderTable();
      },
    });
  } catch (error) {
    statusMessage = error instanceof Error ? error.message : "无法开始游戏";
    renderLobby();
  }
}

function renderTable(): void {
  appRoot.innerHTML = `
    <section class="table-shell">
      <header class="table-header"><span class="eyebrow">RIICHI MAHJONG</span><span class="connection-dot">● ${escapeHtml(statusMessage)}</span></header>
      <div class="mahjong-table">
        <div class="table-center"><strong>东一局</strong><span>等待服务器状态…</span></div>
        <div class="table-player player-top">北家</div><div class="table-player player-left">西家</div><div class="table-player player-right">南家</div>
        <div class="table-player player-bottom">${escapeHtml(room?.players.find((p) => p.id[0] === playerIndex(session!.player))?.nickname ?? "我")}</div>
      </div>
      <div class="hand-panel"><p class="status" id="game-status">${escapeHtml(latestMessage || "等待牌局快照…")}</p><div class="tile-row" id="hand-row"></div></div>
    </section>
  `;
}

function handleServerMessage(message: ServerEnvelope): void {
  const body = message.body;
  if ("StateSnapshot" in body || "StateUpdate" in body) {
    latestMessage = "已收到玩家视角状态快照";
  } else if ("ActionRequired" in body) {
    latestMessage = "轮到你行动";
  } else if ("CallRequired" in body) {
    latestMessage = "请响应当前鸣牌或荣和窗口";
  } else if ("GameOver" in body) {
    latestMessage = "本局已结束";
  } else if ("Error" in body) {
    latestMessage = String(body.Error);
  }
  const status = document.querySelector<HTMLParagraphElement>("#game-status");
  if (status) status.textContent = latestMessage;
}

function escapeHtml(value: string): string {
  return value.replace(/[&<>'"]/g, (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", "'": "&#39;", '"': "&quot;" })[character] ?? character);
}
