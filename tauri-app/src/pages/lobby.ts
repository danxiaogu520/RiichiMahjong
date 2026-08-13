import type { UiState } from "../store";
import { escapeHtml } from "../format";
import { seatName } from "../tiles";
import { appRoot } from "../ui/helpers";

export interface LobbyCallbacks {
  leaveToLobby: () => void;
  toggleReady: () => void;
  startGame: () => void;
  setAiCount: (delta: number) => void;
}

/** 等待大厅 */
export function renderLobby(ui: UiState, callbacks: LobbyCallbacks, own: number): void {
  const room = ui.room;
  if (!ui.session || !room) return;
  const isOwner = room.owner === own;
  const humanPlayers = room.players.filter((player) => !player.is_ai);
  const aiCount = room.players.filter((player) => player.is_ai).length;
  const canStart = isOwner
    && room.players.length === 4
    && humanPlayers.length > 0
    && humanPlayers.every((player) => player.ready);
  const ownPlayer = room.players.find((player) => player.id === own);

  appRoot().innerHTML = `
    <section class="lobby-shell">
      <div class="lobby-topline">
        <span class="eyebrow">Waiting Room</span>
        <button class="btn btn--text" id="leave-button">退出</button>
      </div>
      <h1 class="lobby-title">房间 <span class="room-code">${escapeHtml(room.id)}</span></h1>
      <p class="intro">把房间码发给朋友，四人准备后即可开始。</p>
      <div class="player-list">${room.players.map((player) => `
        <div class="player-row ${player.id === own ? "is-own" : ""}">
          <span class="seat-chip">${seatName(player.id)}</span>
          <span class="player-name">${escapeHtml(player.nickname)}</span>
          <span class="ready-state ${player.ready || player.is_ai ? "is-ready" : ""}">${player.ai_takeover ? "AI 托管" : player.is_ai ? "AI" : !player.connected ? "等待重连" : player.ready ? "已准备" : "等待中"}</span>
        </div>`).join("") || `<div class="empty-state">等待玩家加入…</div>`}</div>
      ${isOwner && !room.started ? `
        <div class="ai-controls">
          <span>AI 补位 <strong>${aiCount}/3</strong></span>
          <div class="ai-stepper">
            <button class="btn btn--square" data-ai-delta="-1" ${aiCount === 0 ? "disabled" : ""}>−</button>
            <button class="btn btn--square" data-ai-delta="1" ${aiCount === 3 ? "disabled" : ""}>＋</button>
          </div>
        </div>` : ""}
      <div class="lobby-actions">
        <button class="btn" id="ready-button">${ownPlayer?.ready ? "取消准备" : "准备"}</button>
        <button class="btn btn--secondary" id="start-button" ${canStart ? "" : "disabled"}>开始半庄</button>
      </div>
      <p class="status-line" id="status" aria-live="polite">${escapeHtml(ui.statusMessage)}</p>
    </section>
  `;

  document.querySelector<HTMLButtonElement>("#ready-button")?.addEventListener("click", callbacks.toggleReady);
  document.querySelector<HTMLButtonElement>("#start-button")?.addEventListener("click", callbacks.startGame);
  document.querySelector<HTMLButtonElement>("#leave-button")?.addEventListener("click", callbacks.leaveToLobby);
  document.querySelectorAll<HTMLButtonElement>("[data-ai-delta]").forEach((button) => {
    button.addEventListener("click", () => callbacks.setAiCount(Number(button.dataset.aiDelta)));
  });
}
