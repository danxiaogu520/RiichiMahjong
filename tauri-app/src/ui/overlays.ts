import type { UiState } from "../store";
import { escapeHtml, roundEndReasonText, pointChangeText } from "../format";
import { seatName } from "../tiles";
import { nicknamesOf } from "./helpers";

/** 单局结算弹窗 */
export function roundResultOverlayHtml(ui: UiState): string {
  const result = ui.roundResult;
  if (!result) return "";
  const { view, nicknames } = result;
  const reason = roundEndReasonText(view.reason);
  return `
    <div class="overlay" id="round-result-overlay">
      <div class="overlay-card">
        <p class="eyebrow">Round Result</p>
        <h2>本局结束 · ${escapeHtml(reason)}</h2>
        ${view.win_details.length ? `<ul class="win-details">${view.win_details.map((detail) => `<li>${escapeHtml(detail)}</li>`).join("")}</ul>` : ""}
        <div class="score-table">${view.point_changes.map((delta, index) => `
          <div class="score-row">
            <span class="seat-chip">${seatName(index)}</span>
            <span class="player-name">${escapeHtml(nicknames[index] ?? seatName(index))}</span>
            <span class="score-delta ${delta > 0 ? "score-plus" : delta < 0 ? "score-minus" : ""}">${pointChangeText(delta)}</span>
          </div>`).join("")}</div>
        <button class="btn" id="round-result-close">知道了</button>
      </div>
    </div>`;
}

/** 半庄终局排名弹窗 */
export function gameOverOverlayHtml(ui: UiState, own: number): string {
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
      <div class="overlay-card">
        <p class="eyebrow">Hanchan Over</p>
        <h2>半庄结束</h2>
        <div class="ranking-list">${rows.map((row) => `
          <div class="ranking-row rank-${row.rank} ${row.playerId === own ? "is-own" : ""}">
            <span class="rank-badge">${row.rank}</span>
            <span class="player-name">${escapeHtml(row.name)}</span>
            <span class="score-delta ${row.points > 0 ? "score-plus" : row.points < 0 ? "score-minus" : ""}">${row.points > 0 ? "+" : ""}${row.points.toLocaleString()}</span>
          </div>`).join("")}</div>
        <div class="lobby-actions">
          <button class="btn btn--secondary" id="game-over-stay">查看牌桌</button>
          <button class="btn" id="game-over-home">返回主页</button>
        </div>
      </div>
    </div>`;
}
