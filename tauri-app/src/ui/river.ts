import type { MeldView } from "../protocol";
import { tileImage, tileBackImage } from "../tiles";

/** 牌河：每行 6 张，先打的在前，立直宣言牌横置 */
export function riverRows(discards: number[], riichiIndex: number): string {
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
 * 副露渲染：
 * - 明副露：鸣到的牌横置，按来源方向插入（下家→最右、对家→中间、上家→最左）；
 * - 暗杠：两端牌背、中间两张翻开。
 */
export function meldImages(meld: MeldView, index: number): string {
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
