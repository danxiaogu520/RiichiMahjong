import type { UiState } from "../store";
import { tileImage, tileLabel } from "../tiles";
import { MELD_NAMES } from "../format";
import { phaseDrawnTile } from "./helpers";

/** 自己的手牌行（含副露、摸牌分离高亮） */
export function ownHandHtml(ui: UiState, own: number): string {
  const game = ui.gameState;
  const hand = game?.players[own]?.hand ?? [];
  const melds = game?.players[own]?.melds ?? [];
  const meldHtml = melds.length
    ? `<div class="own-meld-row">${melds.map((meld) => `
        <span class="meld-chip hand-meld">${meld.tiles.map((tile) => tileImage(tile, "hand-tile")).join("")}<i class="meld-name">${MELD_NAMES[meld.kind] ?? meld.kind}</i></span>`).join("")}</div>`
    : "";
  if (!hand.length) return `${meldHtml}<div class="tile-row">等待快照…</div>`;

  // 服务端下发的 hand 已包含摸牌；把那张牌分离出来放行尾高亮，
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
  const classes = ["tile", drawn ? "is-drawn" : "", legal ? "is-legal" : "is-disabled"].filter(Boolean).join(" ");
  return `<button class="${classes}" ${legal ? `data-action="${action}"` : "disabled"} title="${tileLabel(tile)}">${tileImage(tile, "hand-tile")}</button>`;
}
