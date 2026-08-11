// 牌面 SVG 素材来自 FluffyStuff/riichi-mahjong-tiles（公有领域）。
// 文件位于 public/tiles/，按牌的 type（raw / 4）映射。

const HONOR_FILES = ["Ton", "Nan", "Shaa", "Pei", "Haku", "Hatsu", "Chun"];

/** 牌的 type（0-33）→ SVG 文件名 */
export function tileSvgFile(type: number): string {
  if (type < 27) {
    const suit = ["Man", "Pin", "Sou"][Math.floor(type / 9)];
    const rank = (type % 9) + 1;
    return `${suit}${rank}.svg`;
  }
  return `${HONOR_FILES[type - 27] ?? "Blank"}.svg`;
}

/** 渲染一张牌（raw 编码） */
export function tileImage(raw: number, className = ""): string {
  return `<img class="tile-img ${className}" src="tiles/${tileSvgFile(Math.floor(raw / 4))}" alt="${tileLabel(raw)}" draggable="false" />`;
}

/** 渲染一张按 type 索引的牌（宝牌指示牌等） */
export function tileTypeImage(type: number, className = ""): string {
  return `<img class="tile-img ${className}" src="tiles/${tileSvgFile(type)}" alt="${tileTypeLabel(type)}" draggable="false" />`;
}

/** 牌背 */
export function tileBackImage(className = ""): string {
  return `<img class="tile-img ${className}" src="tiles/Back.svg" alt="牌背" draggable="false" />`;
}

// ─── 文字标签（牌面之外的信息展示仍使用文字） ────────────────

export function tileLabel(raw: number): string {
  return tileTypeLabel(Math.floor(raw / 4));
}

export function tileTypeLabel(type: number): string {
  if (type < 27) return `${(type % 9) + 1}${["万", "筒", "索"][Math.floor(type / 9)]}`;
  return ["东", "南", "西", "北", "白", "发", "中"][type - 27] ?? "?";
}

export function windName(type: number): string {
  return ["东", "南", "西", "北"][type - 27] ?? "东";
}

export function seatName(index: number): string {
  return ["东", "南", "西", "北"][index] ?? "?";
}
