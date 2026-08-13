import type { UiState } from "../store";
import { escapeHtml } from "../format";
import { appRoot } from "../ui/helpers";

export interface JoinCallbacks {
  joinRoom: (form: FormData) => void;
}

/** 加入房间页 */
export function renderJoin(ui: UiState, callbacks: JoinCallbacks, savedServer: string): void {
  appRoot().innerHTML = `
    <section class="join-shell">
      <div class="brand-mark">麻</div>
      <p class="eyebrow">Riichi Mahjong</p>
      <h1>和朋友打<br /><span class="hl">一局半庄</span></h1>
      <p class="intro">连接服务器，输入房间码，准备开始游戏。</p>
      <form class="join-card" id="join-form">
        <label class="field" for="join-server"><span>服务器地址</span><input id="join-server" name="server" value="${escapeHtml(savedServer)}" autocomplete="url" /></label>
        <label class="field" for="join-nickname"><span>昵称</span><input id="join-nickname" name="nickname" placeholder="例如：天凤玩家" maxlength="20" required /></label>
        <label class="field" for="join-room"><span>房间码</span><input id="join-room" name="room" placeholder="留空创建新房间" maxlength="6" /></label>
        <button class="btn" type="submit">进入房间 <span class="btn-arrow">→</span></button>
        <p class="status-line" id="status" aria-live="polite">${escapeHtml(ui.statusMessage)}</p>
      </form>
    </section>
  `;
  document.querySelector<HTMLFormElement>("#join-form")?.addEventListener("submit", (event) => {
    event.preventDefault();
    callbacks.joinRoom(new FormData(event.currentTarget as HTMLFormElement));
  });
}
