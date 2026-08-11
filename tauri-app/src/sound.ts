// 使用 Web Audio API 合成简单音效，不依赖外部音频文件。
// 浏览器要求 AudioContext 在用户手势后创建，因此首次交互（进入房间）时初始化。

let context: AudioContext | undefined;
let enabled = localStorage.getItem("riichi.sound") !== "off";

function ensureContext(): AudioContext | undefined {
  if (!enabled) return undefined;
  if (!context) {
    try {
      context = new AudioContext();
    } catch {
      return undefined;
    }
  }
  if (context.state === "suspended") void context.resume();
  return context;
}

function tone(frequency: number, start: number, duration: number, kind: OscillatorType, volume: number): void {
  const ac = ensureContext();
  if (!ac) return;
  const osc = ac.createOscillator();
  const gain = ac.createGain();
  osc.type = kind;
  osc.frequency.value = frequency;
  const t0 = ac.currentTime + start;
  gain.gain.setValueAtTime(0, t0);
  gain.gain.linearRampToValueAtTime(volume, t0 + 0.008);
  gain.gain.exponentialRampToValueAtTime(0.0001, t0 + duration);
  osc.connect(gain).connect(ac.destination);
  osc.start(t0);
  osc.stop(t0 + duration + 0.02);
}

export type SoundKind = "draw" | "discard" | "call" | "riichi" | "win" | "decline";

export function playSound(kind: SoundKind): void {
  switch (kind) {
    case "draw":
      // 摸牌：短促的木质点击
      tone(880, 0, 0.06, "square", 0.05);
      break;
    case "discard":
      // 打牌：低沉的拍击
      tone(220, 0, 0.09, "square", 0.07);
      tone(110, 0.005, 0.08, "triangle", 0.06);
      break;
    case "call":
      // 鸣牌：双音提示
      tone(440, 0, 0.1, "sine", 0.08);
      tone(554, 0.09, 0.12, "sine", 0.08);
      break;
    case "riichi":
      // 立直：上行的三连音
      tone(523, 0, 0.1, "sine", 0.07);
      tone(659, 0.09, 0.1, "sine", 0.07);
      tone(784, 0.18, 0.16, "sine", 0.08);
      break;
    case "win":
      // 和牌：明亮和弦
      tone(523, 0, 0.32, "triangle", 0.09);
      tone(659, 0, 0.32, "triangle", 0.09);
      tone(784, 0, 0.36, "triangle", 0.09);
      tone(1047, 0.05, 0.3, "sine", 0.06);
      break;
    case "decline":
      // 操作被拒：下行音
      tone(392, 0, 0.12, "sawtooth", 0.05);
      tone(311, 0.1, 0.16, "sawtooth", 0.05);
      break;
  }
}

export function isSoundEnabled(): boolean {
  return enabled;
}

export function setSoundEnabled(value: boolean): void {
  enabled = value;
  localStorage.setItem("riichi.sound", value ? "on" : "off");
  if (!value && context) {
    void context.close();
    context = undefined;
  }
}
