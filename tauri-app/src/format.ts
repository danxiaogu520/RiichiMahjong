import type { GameEventView, RoundEndReasonView } from "./protocol";
import { seatName, tileLabel } from "./tiles";

export function escapeHtml(value: string): string {
  return value.replace(/[&<>'"]/g, (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", "'": "&#39;", '"': "&quot;" })[character] ?? character);
}

export const MELD_NAMES: Record<string, string> = {
  Chi: "吃",
  Pon: "碰",
  Minkan: "大明杠",
  Ankan: "暗杠",
  Kakan: "加杠",
  Ron: "荣和",
  Pass: "跳过",
};

/** 玩家 id → 显示名（优先昵称，其次座位名） */
export function playerName(playerId: number, nicknames: string[]): string {
  return nicknames[playerId] ?? seatName(playerId);
}

/** GameEventView → 中文日志文案 */
export function eventText(event: GameEventView, nicknames: string[]): string {
  const name = (id: number) => playerName(id, nicknames);
  const key = Object.keys(event)[0] as keyof GameEventView;
  switch (key) {
    case "Draw": {
      const payload = (event as { Draw: { player: number; tile: number | null } }).Draw;
      return payload.tile === null
        ? `${name(payload.player)} 摸牌`
        : `${name(payload.player)} 摸到 ${tileLabel(payload.tile)}`;
    }
    case "Discard": {
      const payload = (event as { Discard: { player: number; tile: number; kind: string } }).Discard;
      const way = payload.kind === "Tsumogiri" ? "摸切" : "手切";
      return `${name(payload.player)} 打出 ${tileLabel(payload.tile)}（${way}）`;
    }
    case "Call": {
      const payload = (event as { Call: { player: number; kind: string; called_tile: number | null; from_player: number | null } }).Call;
      const label = MELD_NAMES[payload.kind] ?? payload.kind;
      const from = payload.from_player === null || payload.from_player === payload.player
        ? ""
        : `（来自 ${name(payload.from_player)}）`;
      const tile = payload.called_tile === null ? "" : ` ${tileLabel(payload.called_tile)}`;
      return `${name(payload.player)} ${label}${tile}${from}`;
    }
    case "Pass":
      return `${name((event as { Pass: { player: number } }).Pass.player)} 过`;
    case "Riichi":
      return `${name((event as { Riichi: { player: number } }).Riichi.player)} 立直！`;
    case "Win": {
      const payload = (event as { Win: { winners: number[]; tile: number; kind: string; loser: number | null } }).Win;
      const way = payload.kind === "Tsumo" ? "自摸" : `荣和 ${name(payload.loser ?? -1)}`;
      return `${payload.winners.map(name).join("、")} ${way} ${tileLabel(payload.tile)}！`;
    }
    case "AbortiveDraw": {
      const payload = (event as { AbortiveDraw: { player: number | null; reason: RoundEndReasonView } }).AbortiveDraw;
      return `流局：${roundEndReasonText(payload.reason)}`;
    }
    default:
      return "未知事件";
  }
}

/** RoundEndReasonView → 中文文案 */
export function roundEndReasonText(reason: RoundEndReasonView): string {
  const key = Object.keys(reason)[0];
  switch (key) {
    case "ExhaustiveDraw":
      return "流局";
    case "Win": {
      const payload = (reason as { Win: { winner: number; is_tsumo: boolean } }).Win;
      return payload.is_tsumo ? "自摸和牌" : "荣和";
    }
    case "MultiWin":
      return "多家和牌";
    case "KyuushuKyuuhai":
      return "九种九牌";
    case "SuufonRenda":
      return "四风连打";
    case "SuuchaRiichi":
      return "四家立直";
    case "SuuKantsu":
      return "四杠散了";
    case "Unknown":
      return (reason as { Unknown: string }).Unknown;
    default:
      return "未知结果";
  }
}

/** 点数变化（+2500 / -1000 / ±0） */
export function pointChangeText(delta: number): string {
  if (delta > 0) return `+${delta.toLocaleString()}`;
  if (delta < 0) return `−${Math.abs(delta).toLocaleString()}`;
  return "±0";
}
