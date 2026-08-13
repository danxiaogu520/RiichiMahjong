import type { UiState } from "../store";
import { escapeHtml } from "../format";
import { windName, seatName, tileBackImage, tileTypeImage } from "../tiles";
import { isSoundEnabled } from "../sound";
import { appRoot, nicknamesOf, phasePlayer, phaseName } from "../ui/helpers";
import { riverRows, meldImages } from "../ui/river";
import { ownHandHtml } from "../ui/hand";
import { actionsHtml } from "../ui/actions";
import { analysisHtml } from "../ui/analysis";
import { roundResultOverlayHtml, gameOverOverlayHtml } from "../ui/overlays";

export interface TableCallbacks {
  leaveTable: () => void;
  toggleSound: () => void;
  sendAction: (action: string) => void;
  closeRoundResult: () => void;
  backToHome: () => void;
}

const SEAT_POSITIONS = ["bottom", "right", "top", "left"] as const;

/** 牌桌页 */
export function renderTable(ui: UiState, callbacks: TableCallbacks, own: number): void {
  const game = ui.gameState;
  const names = nicknamesOf(ui.room);
  const honba = game?.honba ?? 0;
  const riichiSticks = game?.riichi_sticks ?? 0;

  appRoot().innerHTML = `
    <section class="table-shell">
      <header class="table-header">
        <span class="eyebrow">Riichi Mahjong</span>
        <span class="connection-status">● ${escapeHtml(ui.statusMessage)}</span>
        <button class="btn btn--text sound-toggle" id="sound-toggle" title="开关音效">${isSoundEnabled() ? "🔊" : "🔇"}</button>
        <button class="btn btn--text" id="table-leave">退出</button>
      </header>
      <div class="mahjong-table">
        ${[0, 1, 2, 3].map((index) => seatHtml(ui, index, names[index], own)).join("")}
        <div class="table-center">
          <span class="center-round">${game ? `${windName(game.wind)}${game.round} 局` : "—"}</span>
          <span class="center-meta">${game ? `${honba} 本场 · 余 ${game.remaining_tiles} 张${riichiSticks > 0 ? ` · 立直棒 ${riichiSticks}` : ""}` : "等待牌局…"}</span>
          <div class="center-dora">${tileBackImage("dora-tile")}${tileBackImage("dora-tile")}${(game?.dora ?? []).map((type) => tileTypeImage(type, "dora-tile")).join("")}</div>
          <span class="center-phase">${game ? phaseName(game) : "待机"}</span>
        </div>
      </div>
      <div class="game-grid">
        <section class="panel hand-panel">
          <div class="panel-heading"><span>我的手牌</span><span>${game?.players[own]?.points?.toLocaleString() ?? "—"} 点</span></div>
          ${ownHandHtml(ui, own)}
        </section>
        <aside class="side-panel">
          <section class="panel action-panel">
            <p class="status-line" id="game-status">${escapeHtml(ui.latestMessage)}</p>
            <div class="action-row"><span class="countdown" id="countdown"></span></div>
            <div id="action-buttons">${actionsHtml(ui)}</div>
          </section>
          <section class="panel analysis-panel">${analysisHtml(ui)}</section>
        </aside>
      </div>
      <section class="panel event-panel">
        <div class="panel-heading"><span>对局记录</span></div>
        <div class="event-list">${ui.events.slice(-60).reverse().map((entry) => `
          <div class="event-line event-line--${entry.kind}"><span class="event-id">#${entry.id}</span>${escapeHtml(entry.text)}</div>`).join("") || `<div class="empty-state">等待对局事件…</div>`}</div>
      </section>
    </section>
    ${roundResultOverlayHtml(ui)}
    ${gameOverOverlayHtml(ui, own)}
  `;

  document.querySelector<HTMLButtonElement>("#table-leave")?.addEventListener("click", callbacks.leaveTable);
  document.querySelector<HTMLButtonElement>("#sound-toggle")?.addEventListener("click", callbacks.toggleSound);
  document.querySelector<HTMLButtonElement>("#round-result-close")?.addEventListener("click", callbacks.closeRoundResult);
  document.querySelector<HTMLButtonElement>("#game-over-stay")?.addEventListener("click", callbacks.closeRoundResult);
  document.querySelector<HTMLButtonElement>("#game-over-home")?.addEventListener("click", callbacks.backToHome);
  document.querySelectorAll<HTMLButtonElement>("[data-action]").forEach((button) => {
    button.addEventListener("click", () => callbacks.sendAction(button.dataset.action!));
  });
}

/**
 * 桌面四边的玩家区域：
 * 区域整体按座位旋转（自己 0° / 右 90° / 对家 180° / 左 90°），
 * 内部统一为：信息条 + 牌河（3 行 × 6 张，向中央让位）+ 副露（右侧）。
 */
function seatHtml(ui: UiState, index: number, name: string, own: number): string {
  const game = ui.gameState;
  const player = game?.players[index];
  const pos = SEAT_POSITIONS[(index - own + 4) % 4];
  const active = phasePlayer(game) === index;
  const discards = player?.discards ?? [];
  const melds = player?.melds ?? [];
  const riichiIndex = player?.riichi_declaration_index ?? -1;
  const windLabel = player ? windName(player.wind) : seatName(index);
  return `
    <div class="table-player seat-${pos} ${active ? "is-active" : ""}">
      <div class="seat-bar">
        <b>${windLabel} ${escapeHtml(name)}${index === own ? "（我）" : ""}</b>
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
