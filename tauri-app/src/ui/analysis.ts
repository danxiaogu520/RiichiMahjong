import type { UiState } from "../store";
import { tileImage, tileTypeImage } from "../tiles";

/** 听牌信息 / 牌效分析面板 */
export function analysisHtml(ui: UiState): string {
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
