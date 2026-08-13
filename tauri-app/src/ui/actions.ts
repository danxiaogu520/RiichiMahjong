import type { UiState } from "../store";
import { playerIndex } from "../protocol";
import { tileLabel } from "../tiles";
import { MELD_NAMES } from "../format";

/**
 * 归一化鸣牌选项的 call_type：服务端对单元变体（荣和）序列化为
 * 字符串 `"Ron"`，结构体变体（吃/碰/杠）才是 `{"Pon": {...}}` 对象，
 * 统一转成 `[变体名, 载荷]` 条目再渲染。
 */
function callTypeEntries(callType: Record<string, unknown> | string): [string, unknown][] {
  return typeof callType === "string" ? [[callType, null]] : Object.entries(callType);
}

/** 主操作区按钮（鸣牌响应 / 回合行动 / 立直待选提示） */
export function actionsHtml(ui: UiState): string {
  if (!ui.session) return "";

  if (ui.callRequest?.player === playerIndex(ui.session.player)) {
    return ui.callRequest.options.map((option, index) =>
      callTypeEntries(option.call_type).map(([kind, payload]) => {
        const cls = kind === "Ron" ? "btn--ron" : kind === "Chi" ? "btn--chi" : kind === "Pon" ? "btn--pon" : "btn--kan";
        return `<button class="btn ${cls}" data-action="call:${kind}:${index}">${MELD_NAMES[kind] ?? kind}${callTiles(payload)}</button>`;
      }).join("")
    ).join("") + `<button class="btn btn--pass" data-action="call:Pass:-1">跳过</button>`;
  }

  if (ui.actionRequest?.player !== playerIndex(ui.session.player)) return "";
  const request = ui.actionRequest;

  // 立直待选状态：禁止其他操作，只保留提示和取消，打牌方式 = 点手牌。
  if (ui.riichiPending) {
    return `<span class="riichi-hint">请点击手牌中要打出的牌（立直宣言）</span><button class="btn btn--pass" data-action="RiichiCancel">取消立直</button>`;
  }

  const buttons: string[] = [];
  if (request.can_tsumo) buttons.unshift(`<button class="btn btn--tsumo" data-action="Tsumo">自摸</button>`);
  if (request.can_riichi) buttons.push(`<button class="btn btn--riichi" data-action="Riichi">立直</button>`);
  buttons.push(...request.ankan_options.map((tile) => `<button class="btn btn--kan" data-action="ankan:${tile}">暗杠 ${tileLabel(tile)}</button>`));
  buttons.push(...request.kakan_options.map(([index, tile]) => `<button class="btn btn--kan" data-action="kakan:${index}:${tile}">加杠 ${tileLabel(tile)}</button>`));
  if (request.can_kyuushu) buttons.push(`<button class="btn btn--ryukyoku" data-action="KyuushuKyuuhai">九种九牌</button>`);
  return buttons.join("") || `<span class="status-line">等待操作…</span>`;
}

function callTiles(payload: unknown): string {
  if (!payload || typeof payload !== "object" || !("hand_tiles" in payload)) return "";
  const tiles = (payload as { hand_tiles: number[] }).hand_tiles;
  return tiles.length ? ` · ${tiles.map(tileLabel).join("/")}` : "";
}
