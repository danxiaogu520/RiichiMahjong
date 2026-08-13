// CSS 链路自检（临时）
import "./style.css";
const el = document.createElement("div");
el.className = "mahjong-table";
document.body.appendChild(el);
const cs = getComputedStyle(el);
const link = document.querySelector("style, link[rel=stylesheet]");
document.body.innerHTML = `<pre style="font:14px monospace;padding:20px">` +
  `table bg: ${cs.backgroundColor}\nwidth: ${cs.width}\n` +
  `style tags: ${document.querySelectorAll("style").length}\n` +
  `first style len: ${document.querySelector("style")?.textContent?.length ?? "none"}\n` +
  `link: ${link ? link.outerHTML.slice(0, 120) : "none"}\n` +
  `body bg: ${getComputedStyle(document.body).backgroundColor}</pre>`;
