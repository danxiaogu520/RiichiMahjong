// 一次性脚本：从 FluffyStuff/riichi-mahjong-tiles（公有领域素材）下载立直麻将牌 SVG。
import { writeFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";

const BASE = "https://raw.githubusercontent.com/FluffyStuff/riichi-mahjong-tiles/master/Regular";
const FILES = [
  "Man1", "Man2", "Man3", "Man4", "Man5", "Man6", "Man7", "Man8", "Man9",
  "Pin1", "Pin2", "Pin3", "Pin4", "Pin5", "Pin6", "Pin7", "Pin8", "Pin9",
  "Sou1", "Sou2", "Sou3", "Sou4", "Sou5", "Sou6", "Sou7", "Sou8", "Sou9",
  "Man5-Dora", "Pin5-Dora", "Sou5-Dora",
  "Ton", "Nan", "Shaa", "Pei", "Haku", "Hatsu", "Chun",
  "Back", "Blank", "Front",
];

const outDir = join(import.meta.dirname, "..", "public", "tiles");
mkdirSync(outDir, { recursive: true });

for (const name of FILES) {
  const response = await fetch(`${BASE}/${name}.svg`);
  if (!response.ok) {
    console.error(`下载失败: ${name} (${response.status})`);
    process.exitCode = 1;
    continue;
  }
  writeFileSync(join(outDir, `${name}.svg`), await response.text());
  console.log(`已下载 ${name}.svg`);
}
console.log(`完成，共 ${FILES.length} 个文件 -> ${outDir}`);
