import "./style.css";

const app = document.querySelector<HTMLDivElement>("#app");

if (!app) {
  throw new Error("应用根节点不存在");
}

app.innerHTML = `
  <section class="shell">
    <div class="brand-mark">麻</div>
    <p class="eyebrow">RIICHI MAHJONG</p>
    <h1>和朋友打<br /><span>一局半庄</span></h1>
    <p class="intro">连接服务器，输入房间码，准备开始游戏。</p>
    <form class="join-card" id="join-form">
      <label>
        <span>服务器地址</span>
        <input name="server" value="http://127.0.0.1:3000" autocomplete="url" />
      </label>
      <label>
        <span>昵称</span>
        <input name="nickname" placeholder="例如：天凤玩家" maxlength="20" required />
      </label>
      <label>
        <span>房间码</span>
        <input name="room" placeholder="留空创建新房间" maxlength="6" />
      </label>
      <button type="submit">进入房间 <span>→</span></button>
      <p class="status" id="status" aria-live="polite">服务器尚未连接</p>
    </form>
  </section>
`;

document.querySelector<HTMLFormElement>("#join-form")?.addEventListener("submit", (event) => {
  event.preventDefault();
  const status = document.querySelector<HTMLParagraphElement>("#status");
  if (status) {
    status.textContent = "房间连接功能即将接入";
  }
});
