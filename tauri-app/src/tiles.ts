// 牌面 SVG 素材来自 FluffyStuff/riichi-mahjong-tiles（公有领域）。
// 文件位于 public/tiles/，按牌的 type（raw / 4）映射。
// 牌面 SVG 只有图案（透明背景），需要叠加 Front.svg（白色牌身）打底。

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

function tileFace(svgFile: string, alt: string, className: string): string {
  return `<span class="tile-face ${className}"><img class="tile-front" src="tiles/Front.svg" alt="" draggable="false" /><img class="tile-print" src="tiles/${svgFile}" alt="${alt}" draggable="false" /></span>`;
}

/** 渲染一张牌（raw 编码），白色牌身 + 图案叠加 */
export function tileImage(raw: number, className = ""): string {
  const type = Math.floor(raw / 4);
  return tileFace(tileSvgFile(type), tileLabel(raw), className);
}

/** 渲染一张按 type 索引的牌（宝牌等） */
export function tileTypeImage(type: number, className = ""): string {
  return tileFace(tileSvgFile(type), tileTypeLabel(type), className);
}

/** 牌背（单图，无需牌身叠加） */
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
