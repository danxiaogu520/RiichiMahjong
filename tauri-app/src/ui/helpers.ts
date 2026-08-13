import type { GameStateView, RoomInfo } from "../protocol";
import { seatName } from "../tiles";

/** 应用根节点 */
export function appRoot(): HTMLElement {
  const root = document.querySelector<HTMLElement>("#app");
  if (!root) throw new Error("应用根节点不存在");
  return root;
}

/** 四个座位的显示名（优先昵称，AI/托管加注） */
export function nicknamesOf(room: RoomInfo | undefined): string[] {
  return [0, 1, 2, 3].map((index) => {
    const player = room?.players.find((candidate) => candidate.id === index);
    if (!player) return seatName(index);
    if (player.ai_takeover) return `${player.nickname}(AI托管)`;
    return player.is_ai ? "AI" : player.nickname;
  });
}

/** 当前阶段行动的玩家 */
export function phasePlayer(gameState?: GameStateView): number | undefined {
  if (!gameState || typeof gameState.phase === "string") return undefined;
  return Object.values(gameState.phase)[0].player;
}

/** ActionPhase 中玩家刚摸的牌 */
export function phaseDrawnTile(gameState?: GameStateView): number | null | undefined {
  if (!gameState || typeof gameState.phase === "string" || !("ActionPhase" in gameState.phase)) {
    return undefined;
  }
  return gameState.phase.ActionPhase.drawn_tile;
}

/** 阶段的中文名 */
export function phaseName(gameState?: GameStateView): string {
  const phase = gameState?.phase;
  if (!phase || typeof phase === "string") return "本局结束";
  if ("DrawPhase" in phase) return phase.DrawPhase.position === "Rinshan" ? "岭上摸牌" : "摸牌";
  if ("ActionPhase" in phase) return "行动";
  if ("ResponsePhase" in phase) return "响应";
  return "抢杠";
}
