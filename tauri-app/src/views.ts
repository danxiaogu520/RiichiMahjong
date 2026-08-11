import type {
  ActionRequest,
  CallRequest,
  GameOverView,
  GameStateView,
  JoinInfo,
  MeldView,
  PlayerId,
  RoundResultView,
  RoomInfo,
} from "./protocol";
import { playerIndex } from "./protocol";
import { tileImage, tileTypeImage, tileBackImage, tileLabel, windName, seatName } from "./tiles";
import {
  escapeHtml,
  MELD_NAMES,
  playerName,
  roundEndReasonText,
  pointChangeText,
  eventText,
} from "./format";
import { playSound, isSoundEnabled, setSoundEnabled } from "./sound";

// ─── 共享 UI 状态（main.ts 负责更新并触发渲染） ──────────────

export interface LogEntry {
  id: number;
  text: string;
  kind: "draw" | "discard" | "call" | "riichi" | "win" | "other";
}

export interface UiState {
  session?: JoinInfo;
  room?: RoomInfo;
  statusMessage: string;
  latestMessage: string;
  gameState?: GameStateView;
  actionRequest?: ActionRequest;
  callRequest?: CallRequest;
  actionDeadline: number;
  /** 立直待选状态：点了立直按钮后等待从手牌选牌打出 */
  riichiPending: boolean;
  events: LogEntry[];
  roundResult?: { view: RoundResultView; nicknames: string[] };
  gameOver?: GameOverView;
}

export interface UiCallbacks {
  joinRoom: (form: FormData) => void;
  leaveToLobby: () => void;
  toggleReady: () => void;
  startGame: () => void;
  setAiCount: (delta: number) => void;
  sendAction: (action: string) => void;
  leaveTable: () => void;
  closeRoundResult: () => void;
  backToHome: () => void;
  toggleSound: () => void;
}

function nicknamesOf(room: RoomInfo | undefined): string[] {
  return [0, 1, 2, 3].map((index) => {
    const player = room?.players.find((candidate) => candidate.id === index);
    if (!player) return seatName(index);
    if (player.ai_takeover) return `${player.nickname}(AI托管)`;
    return player.is_ai ? "AI" : player.nickname;
  });
}

export { nicknamesOf };

function ownIndex(ui: UiState): number {
  return ui.session ? playerIndex(ui.session.player) : 0;
}

function phasePlayer(gameState?: GameStateView): number | undefined {
  if (!gameState || typeof gameState.phase === "string") return undefined;
  return Object.values(gameState.phase)[0].player;
}

function phaseDrawnTile(gameState?: GameStateView): number | null | undefined {
  if (!gameState || typeof gameState.phase === "string" || !("ActionPhase" in gameState.phase)) {
    return undefined;
  }
  return gameState.phase.ActionPhase.drawn_tile;
}

function phaseName(gameState?: GameStateView): string {
  const phase = gameState?.phase;
  if (!phase || typeof phase === "string") return "本局结束";
  if ("DrawPhase" in phase) return phase.DrawPhase.position === "Rinshan" ? "岭上摸牌" : "摸牌";
  if ("ActionPhase" in phase) return "行动";
  if ("ResponsePhase" in phase) return "响应";
  return "抢杠";
}

// ─── 加入房间页 ───────────────────────────────────────────────

export function renderJoin(ui: UiState, callbacks: UiCallbacks, savedServer: string): void {
  const root = appRoot();
  root.innerHTML = `
    <section class="shell">
      <div class="brand-mark">麻</div>
      <p class="eyebrow">RIICHI MAHJONG</p>
      <h1>和朋友打<br /><span>一局半庄</span></h1>
      <p class="intro">连接服务器，输入房间码，准备开始游戏。</p>
      <form class="join-card" id="join-form">
        <label><span>服务器地址</span><input name="server" value="${escapeHtml(savedServer)}" autocomplete="url" /></label>
        <label><span>昵称</span><input name="nickname" placeholder="例如：天凤玩家" maxlength="20" required /></label>
        <label><span>房间码</span><input name="room" placeholder="留空创建新房间" maxlength="6" /></label>
        <button type="submit">进入房间 <span>→</span></button>
        <p class="status" id="status" aria-live="polite">${escapeHtml(ui.statusMessage)}</p>
      </form>
    </section>
  `;
  root.querySelector<HTMLFormElement>("#join-form")?.addEventListener("submit", (event) => {
    event.preventDefault();
    callbacks.joinRoom(new FormData(event.currentTarget as HTMLFormElement));
  });
}

// ─── 大厅 ─────────────────────────────────────────────────────

export function renderLobby(ui: UiState, callbacks: UiCallbacks): void {
  const room = ui.room;
  if (!ui.session || !room) return;
  const own = ownIndex(ui);
  const isOwner = room.owner === own;
  const humanPlayers = room.players.filter((player) => !player.is_ai);
  const aiCount = room.players.filter((player) => player.is_ai).length;
  const canStart = isOwner
    && room.players.length === 4
    && humanPlayers.length > 0
    && humanPlayers.every((player) => player.ready);

  appRoot().innerHTML = `
    <section class="shell lobby-shell">
      <div class="lobby-topline"><span class="eyebrow">WAITING ROOM</span><button class="text-button" id="leave-button">退出</button></div>
      <h1 class="lobby-title">房间 <span>${escapeHtml(room.id)}</span></h1>
      <p class="intro">把房间码发给朋友，四人准备后即可开始。</p>
      <div class="player-list" id="player-list">${room.players.map((player) => `
        <div class="player-row ${player.id === own ? "own-player" : ""}">
          <span class="seat">${seatName(player.id)}</span>
          <span class="player-name">${escapeHtml(player.nickname)}</span>
          <span class="ready-state ${player.ready || player.is_ai ? "is-ready" : ""}">${player.ai_takeover ? "AI 托管" : player.is_ai ? "AI" : !player.connected ? "等待重连" : player.ready ? "已准备" : "等待中"}</span>
        </div>`).join("") || `<div class="empty-state">等待玩家加入…</div>`}</div>
      ${isOwner && !room.started ? `<div class="ai-controls"><span>AI 补位 <strong>${aiCount}/3</strong></span><div><button class="small-button" data-ai-delta="-1" ${aiCount === 0 ? "disabled" : ""}>−</button><button class="small-button" data-ai-delta="1" ${aiCount === 3 ? "disabled" : ""}>＋</button></div></div>` : ""}
      <div class="lobby-actions">
        <button id="ready-button">${room.players.find((player) => player.id === own)?.ready ? "取消准备" : "准备"}</button>
        <button class="secondary-button" id="start-button" ${canStart ? "" : "disabled"}>开始半庄</button>
      </div>
      <p class="status" id="status" aria-live="polite">${escapeHtml(ui.statusMessage)}</p>
    </section>
  `;
  document.querySelector<HTMLButtonElement>("#ready-button")?.addEventListener("click", callbacks.toggleReady);
  document.querySelector<HTMLButtonElement>("#start-button")?.addEventListener("click", callbacks.startGame);
  document.querySelectorAll<HTMLButtonElement>("[data-ai-delta]").forEach((button) => {
    button.addEventListener("click", () => callbacks.setAiCount(Number(button.dataset.aiDelta)));
  });
  document.querySelector<HTMLButtonElement>("#leave-button")?.addEventListener("click", callbacks.leaveToLobby);
}

// ─── 牌桌 ─────────────────────────────────────────────────────

export function renderTable(ui: UiState, callbacks: UiCallbacks): void {
  const game = ui.gameState;
  const own = ownIndex(ui);
  const names = nicknamesOf(ui.room);
  const honba = game?.honba ?? 0;
  const riichiSticks = game?.riichi_sticks ?? 0;

  appRoot().innerHTML = `
    <section class="table-shell">
      <header class="table-header">
        <span class="eyebrow">RIICHI MAHJONG</span>
        <span class="connection-dot">● ${escapeHtml(ui.statusMessage)}</span>
        <button class="text-button sound-toggle" id="sound-toggle" title="开关音效">${isSoundEnabled() ? "🔊" : "🔇"}</button>
        <button class="text-button" id="table-leave">退出</button>
      </header>
      <div class="mahjong-table">
        ${[0, 1, 2, 3].map((index) => renderTableSeat(ui, index, names[index], own)).join("")}
        <div class="table-status">${game ? `${windName(game.wind)}${game.round}局 · ${honba} 本场 · 剩余 ${game.remaining_tiles} 张${riichiSticks > 0 ? ` · 立直棒 ${riichiSticks}` : ""} · ${phaseName(game)}` : "等待牌局…"}</div>
        <div class="wanpai-area">${tileBackImage("wanpai-tile")}${tileBackImage("wanpai-tile")}${(game?.dora ?? []).map((type) => tileTypeImage(type, "wanpai-tile")).join("")}</div>
      </div>
      <div class="game-grid">
        <section class="hand-panel">
          <div class="panel-heading"><span>我的手牌</span><span>${game?.players[own]?.points?.toLocaleString() ?? "—"} 点</span></div>
          ${renderOwnHand(ui)}
        </section>
        <aside class="side-panel">
          <section class="action-panel"><p class="status" id="game-status">${escapeHtml(ui.latestMessage)}</p><div class="action-row"><span id="countdown"></span></div><div id="action-buttons">${renderActions(ui)}</div></section>
          <section class="analysis-panel">${renderAnalysis(ui)}</section>
        </aside>
      </div>
      <section class="event-panel">
        <div class="panel-heading"><span>对局记录</span></div>
        <div class="event-list">${ui.events.slice(-60).reverse().map((entry) => `<div class="event-line event-${entry.kind}"><span class="event-id">#${entry.id}</span>${escapeHtml(entry.text)}</div>`).join("") || `<div class="empty-state">等待对局事件…</div>`}</div>
      </section>
    </section>
    ${renderRoundResultOverlay(ui)}
    ${renderGameOverOverlay(ui)}
  `;
  document.querySelector<HTMLButtonElement>("#table-leave")?.addEventListener("click", callbacks.leaveTable);
  document.querySelector<HTMLButtonElement>("#sound-toggle")?.addEventListener("click", callbacks.toggleSound);
  document.querySelectorAll<HTMLButtonElement>("[data-action]").forEach((button) => {
    button.addEventListener("click", () => callbacks.sendAction(button.dataset.action!));
  });
  document.querySelector<HTMLButtonElement>("#round-result-close")?.addEventListener("click", callbacks.closeRoundResult);
  document.querySelector<HTMLButtonElement>("#game-over-stay")?.addEventListener("click", callbacks.closeRoundResult);
  document.querySelector<HTMLButtonElement>("#game-over-home")?.addEventListener("click", callbacks.backToHome);
}

/**
 * 桌面四边的玩家区域（参考 Mortal log-viewer 布局）：
 * 区域整体按座位旋转（自己 0° / 右 90° / 对家 180° / 左 90°），
 * 内部统一为：信息条 + 牌河（3 行 × 6 张，向中央让位）+ 副露（右侧）。
 */
function renderTableSeat(ui: UiState, index: number, name: string, own: number): string {
  const game = ui.gameState;
  const player = game?.players[index];
  // 自己永远在下方，其余按相对座位旋转（Mortal 的 viewpoint 视角）。
  const pos = ["bottom", "right", "top", "left"][(index - own + 4) % 4];
  const active = phasePlayer(game) === index ? " active-seat" : "";
  const discards = player?.discards ?? [];
  const melds = player?.melds ?? [];
  const riichiIndex = player?.is_riichi ? discards.length - 1 : -1;
  // 牌桌内按真实座风显示（庄家为东），开局前回退到座位号。
  const windLabel = player ? windName(player.wind) : seatName(index);
  return `
    <div class="table-player player-${pos}${active}">
      <div class="seat-bar">
        <b>${windLabel} ${name}${index === own ? "（我）" : ""}</b>
        <span class="seat-points">${player?.points?.toLocaleString() ?? "—"}</span>
        ${player?.is_riichi ? `<span class="riichi-badge">立直</span>` : ""}
        <span class="seat-count">${player ? `${player.hand_count} 枚` : ""}</span>
      </div>
      <div class="seat-body">
        <div class="seat-river">${riverRows(discards, riichiIndex) || `<span class="river-empty">—</span>`}</div>
        ${melds.length ? `<div class="seat-furos">${melds.map((meld) => `<span class="meld-chip">${meldImages(meld, index)}</span>`).join("")}</div>` : ""}
      </div>
    </div>`;
}

/** 牌河：每行 6 张，先打的在前，立直宣言牌横置 */
function riverRows(discards: number[], riichiIndex: number): string {
  const rows: string[] = [];
  for (let start = 0; start < discards.length; start += 6) {
    rows.push(`<div class="river-row">${discards.slice(start, start + 6).map((tile, i) => {
      const global = start + i;
      return global === riichiIndex
        ? `<span class="laid-tile">${tileImage(tile, "river-tile")}</span>`
        : tileImage(tile, "river-tile");
    }).join("")}</div>`);
  }
  return rows.join("");
}

/**
 * 副露渲染（参考 Mortal）：
 * - 明副露：鸣到的牌横置，按来源方向插入（下家→最右、对家→中间、上家→最左）；
 * - 暗杠：两端牌背、中间两张翻开。
 */
function meldImages(meld: MeldView, index: number): string {
  const tiles = meld.tiles;
  if (meld.kind === "Ankan" && tiles.length >= 4) {
    return `${tileBackImage("meld-tile")}${tileImage(tiles[0], "meld-tile")}${tileImage(tiles[1], "meld-tile")}${tileBackImage("meld-tile")}`;
  }
  const dir = meld.from_player !== null && meld.from_player !== index ? (meld.from_player - index + 4) % 4 : 0;
  const laidPos = [null, 3, 1, 0][dir] as number | null;
  const taken = tiles[tiles.length - 1];
  const handTiles = tiles.slice(0, -1);
  const parts: string[] = [];
  let handIndex = 0;
  for (let i = 0; i < tiles.length; i += 1) {
    if (laidPos !== null && i === laidPos) {
      parts.push(`<span class="laid-tile">${tileImage(taken, "meld-tile")}</span>`);
    } else {
      parts.push(tileImage(handTiles[handIndex] ?? taken, "meld-tile"));
      handIndex += 1;
    }
  }
  return parts.join("");
}

function renderOwnHand(ui: UiState): string {
  const game = ui.gameState;
  const own = ownIndex(ui);
  const hand = game?.players[own]?.hand ?? [];
  const melds = game?.players[own]?.melds ?? [];
  const meldHtml = melds.length ? `<div class="own-meld-row">${melds.map((meld) => `
    <span class="meld-chip hand-meld">${meld.tiles.map((tile) => tileImage(tile, "hand-tile")).join("")}<i>${MELD_NAMES[meld.kind] ?? meld.kind}</i></span>`).join("")}</div>` : "";
  if (!hand.length) return `${meldHtml}<div class="tile-row">等待快照…</div>`;
  // 服务端下发的 hand 已包含当前玩家的摸牌；把那张牌分离出来放行尾高亮，
  // 避免与 phase.drawn_tile 重复渲染导致牌数多一张。
  const drawnRaw = phaseDrawnTile(game);
  const drawnIndex = drawnRaw !== undefined && drawnRaw !== null ? hand.indexOf(drawnRaw) : -1;
  const parts: string[] = [];
  hand.forEach((tile, index) => {
    if (index === drawnIndex) return;
    parts.push(handTileButton(ui, tile, false));
  });
  if (drawnIndex >= 0) {
    parts.push(`<span class="draw-gap"></span>`);
    parts.push(handTileButton(ui, hand[drawnIndex], true));
  }
  return `${meldHtml}<div class="tile-row">${parts.join("")}</div>`;
}

function handTileButton(ui: UiState, tile: number, drawn: boolean): string {
  // 立直待选：只能点立直可打的牌（点击发送 RiichiDiscard）；否则只能点普通可打牌。
  const legal = ui.riichiPending
    ? (ui.actionRequest?.riichi_options.includes(tile) ?? false)
    : (ui.actionRequest?.discard_options.includes(tile) ?? false);
  const action = ui.riichiPending ? `riichi:${tile}` : `discard:${tile}`;
  const className = `tile ${drawn ? "drawn-tile" : ""} ${legal ? "legal-tile" : "disabled-tile"}`;
  return `<button class="${className}" ${legal ? `data-action="${action}"` : "disabled"} title="${tileLabel(tile)}">${tileImage(tile, "hand-tile")}</button>`;
}

function renderActions(ui: UiState): string {
  if (ui.callRequest?.player === playerIndex(ui.session!.player)) {
    return ui.callRequest.options.map((option, index) => Object.entries(option.call_type).map(([kind, payload]) => {
      const cls = kind === "Ron" ? "btn-ron" : kind === "Chi" ? "btn-chi" : kind === "Pon" ? "btn-pon" : "btn-kan";
      return `<button class="${cls}" data-action="call:${kind}:${index}">${MELD_NAMES[kind] ?? kind}${callTiles(payload)}</button>`;
    }).join("")).join("")
      + `<button class="btn-pass" data-action="call:Pass:-1">跳过</button>`;
  }
  if (ui.actionRequest?.player !== playerIndex(ui.session!.player)) return "";
  const request = ui.actionRequest;
  // 立直待选状态：禁止其他操作，只保留提示和取消，打牌方式 = 点手牌。
  if (ui.riichiPending) {
    return `<span class="riichi-hint">请点击手牌中要打出的牌（立直宣言）</span><button class="btn-pass" data-action="RiichiCancel">取消立直</button>`;
  }
  const buttons: string[] = [];
  if (request.can_tsumo) buttons.unshift(`<button class="btn-tsumo" data-action="Tsumo">自摸</button>`);
  if (request.can_riichi) buttons.push(`<button class="btn-riichi" data-action="Riichi">立直</button>`);
  buttons.push(...request.ankan_options.map((tile) => `<button class="btn-kan" data-action="ankan:${tile}">暗杠 ${tileLabel(tile)}</button>`));
  buttons.push(...request.kakan_options.map(([index, tile]) => `<button class="btn-kan" data-action="kakan:${index}:${tile}">加杠 ${tileLabel(tile)}</button>`));
  if (request.can_kyuushu) buttons.push(`<button class="btn-ryukyoku" data-action="KyuushuKyuuhai">九种九牌</button>`);
  return buttons.join("") || `<span class="status">等待操作…</span>`;
}

function callTiles(payload: unknown): string {
  if (!payload || typeof payload !== "object" || !("hand_tiles" in payload)) return "";
  const tiles = (payload as { hand_tiles: number[] }).hand_tiles;
  return tiles.length ? ` · ${tiles.map(tileLabel).join("/")}` : "";
}

/** 听牌信息 / 牌效分析面板 */
function renderAnalysis(ui: UiState): string {
  const game = ui.gameState;
  const tenpai = game?.tenpai_info;
  const analysis = game?.analysis;
  if (tenpai && tenpai.waits.length) {
    return `
      <div class="panel-heading"><span>听牌信息</span>${tenpai.is_furiten ? `<span class="flag-furiten">振听</span>` : ""}</div>
      <div class="tenpai-list">${tenpai.waits.map((wait) => `
        <div class="tenpai-row">
          ${tileTypeImage(wait.tile_type, "tenpai-tile")}
          <span class="tenpai-remaining">剩 ${wait.remaining} 张</span>
          ${wait.is_no_yaku ? `<span class="flag-no-yaku">无役</span>` : ""}
        </div>`).join("")}</div>`;
  }
  if (analysis && analysis.discard_options.length) {
    return `
      <div class="panel-heading"><span>牌效分析</span><span>当前向听 ${analysis.current_shanten}</span></div>
      <div class="analysis-list">${analysis.discard_options.slice(0, 5).map((option) => `
        <div class="analysis-row">
          ${tileImage(option.tile, "analysis-tile")}
          <span class="analysis-shanten">向听 ${option.shanten}</span>
          <span class="analysis-count">进张 ${option.acceptance_count}</span>
          <span class="analysis-count">改良 ${option.improvement_count}</span>
        </div>`).join("")}</div>`;
  }
  return "";
}

// ─── 结算与终局 ───────────────────────────────────────────────

function renderRoundResultOverlay(ui: UiState): string {
  const result = ui.roundResult;
  if (!result) return "";
  const { view, nicknames } = result;
  const reason = roundEndReasonText(view.reason);
  return `
    <div class="overlay" id="round-result-overlay">
      <div class="overlay-card round-result-card">
        <p class="eyebrow">ROUND RESULT</p>
        <h2>本局结束 · ${escapeHtml(reason)}</h2>
        ${view.win_details.length ? `<ul class="win-details">${view.win_details.map((detail) => `<li>${escapeHtml(detail)}</li>`).join("")}</ul>` : ""}
        <div class="score-table">${view.point_changes.map((delta, index) => `
          <div class="score-row">
            <span class="seat">${seatName(index)}</span>
            <span class="player-name">${escapeHtml(nicknames[index] ?? seatName(index))}</span>
            <span class="score-delta ${delta > 0 ? "score-plus" : delta < 0 ? "score-minus" : ""}">${pointChangeText(delta)}</span>
          </div>`).join("")}</div>
        <button id="round-result-close">知道了</button>
      </div>
    </div>`;
}

function renderGameOverOverlay(ui: UiState): string {
  const over = ui.gameOver;
  if (!over) return "";
  const names = nicknamesOf(ui.room);
  const rows = over.ranking.map((playerId, rank) => ({
    rank: rank + 1,
    playerId,
    name: names[playerId] ?? seatName(playerId),
    points: over.scores[playerId],
  }));
  return `
    <div class="overlay" id="game-over-overlay">
      <div class="overlay-card game-over-card">
        <p class="eyebrow">HANCHAN OVER</p>
        <h2>半庄结束</h2>
        <div class="ranking-list">${rows.map((row) => `
          <div class="ranking-row rank-${row.rank} ${row.playerId === ownIndex(ui) ? "own-player" : ""}">
            <span class="rank-badge">${row.rank}</span>
            <span class="player-name">${escapeHtml(row.name)}</span>
            <span class="score-delta ${row.points > 0 ? "score-plus" : row.points < 0 ? "score-minus" : ""}">${row.points > 0 ? "+" : ""}${row.points.toLocaleString()}</span>
          </div>`).join("")}</div>
        <div class="lobby-actions">
          <button class="secondary-button" id="game-over-stay">查看牌桌</button>
          <button id="game-over-home">返回主页</button>
        </div>
      </div>
    </div>`;
}

function appRoot(): HTMLDivElement {
  const root = document.querySelector<HTMLDivElement>("#app");
  if (!root) throw new Error("应用根节点不存在");
  return root;
}

export function playEventSound(kind: string): void {
  if (kind === "draw") playSound("draw");
  else if (kind === "discard") playSound("discard");
  else if (kind === "call") playSound("call");
  else if (kind === "riichi") playSound("riichi");
  else if (kind === "win") playSound("win");
}

export function toggleSound(): boolean {
  setSoundEnabled(!isSoundEnabled());
  return isSoundEnabled();
}
