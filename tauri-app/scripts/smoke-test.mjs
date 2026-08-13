// 端到端冒烟测试：启动服务器，走完建房→加入→AI→开局流程，
// 验证 WebSocket 消息中包含 analysis / Event / win_details 等新数据。
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { setTimeout as sleep } from "node:timers/promises";

const BASE = "http://127.0.0.1:13600";
const projectRoot = fileURLToPath(new URL("../..", import.meta.url));
const cargoHome = join(homedir(), ".cargo", "bin", "cargo.exe");
const cargoBin = process.env.CARGO || (existsSync(cargoHome) ? cargoHome : "cargo");
const server = spawn(cargoBin, ["run", "-p", "riichi-server"], { cwd: projectRoot, stdio: ["ignore", "pipe", "pipe"] });

let log = "";
server.stdout.on("data", (chunk) => { log += chunk; });
server.stderr.on("data", (chunk) => { log += chunk; });

async function waitForServer(attempts = 300) {
  for (let i = 0; i < attempts; i++) {
    try {
      const response = await fetch(`${BASE}/rooms`, { method: "POST" });
      if (response.ok) return;
    } catch { /* 服务器未就绪 */ }
    await sleep(250);
  }
  throw new Error(`服务器未就绪:\n${log}`);
}

async function post(path, body) {
  const response = await fetch(`${BASE}${path}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body ?? {}),
  });
  if (!response.ok) throw new Error(`${path} -> ${response.status} ${await response.text()}`);
  return response.json();
}

const results = { analysis: 0, tenpai: 0, events: 0, winDetails: false, roundResult: 0, handChecks: 0, handErrors: [] };

try {
  await waitForServer();
  console.log("✅ 服务器已就绪");
  const room = await post("/rooms", {});
  console.log("✅ 房间已创建", room.id);
  const joined = await post(`/rooms/${room.id}/join`, { nickname: "冒烟测试" });
  console.log("✅ 已加入，座位", joined.player);
  await post(`/rooms/${room.id}/ai`, { token: joined.token, count: 3 });
  console.log("✅ AI 补位完成");
  await post(`/rooms/${room.id}/ready`, { token: joined.token, ready: true });
  await post(`/rooms/${room.id}/start`, { token: joined.token });
  console.log("✅ 对局已开始");

  const ws = new WebSocket(`ws://127.0.0.1:13600/ws?room_id=${room.id}&token=${joined.token}&last_event_id=0`);
  const messages = [];
  let lastSeq = 0;
  let commandId = 0;
  ws.onmessage = (event) => {
    const message = JSON.parse(event.data);
    if (typeof message.seq === "number") lastSeq = message.seq;
    const body = message.body;
    if ("StateSnapshot" in body || "StateUpdate" in body) {
      const view = body.StateSnapshot ?? body.StateUpdate;
      if (view.analysis?.discard_options?.length) results.analysis += 1;
      if (view.tenpai_info?.waits?.length) results.tenpai += 1;
      // 牌数校验：摸牌后手牌应为 14 张，且 phase.drawn_tile 必须包含在 hand 中。
      const hand = view.players[0]?.hand;
      if (Array.isArray(hand) && hand.length >= 13) {
        results.handChecks += 1;
        const phase = view.phase;
        if (phase && typeof phase === "object" && "ActionPhase" in phase) {
          const drawn = phase.ActionPhase.drawn_tile;
          if (drawn !== null && drawn !== undefined && !hand.includes(drawn)) {
            results.handErrors.push(`drawn ${drawn} 不在 hand ${hand.length} 张中`);
          }
        }
        if (hand.length > 14) {
          results.handErrors.push(`手牌 ${hand.length} 张超过 14`);
        }
      }
    }
    if ("Event" in body) results.events += 1;
    if ("RoundResult" in body) {
      results.roundResult += 1;
      if (body.RoundResult.win_details?.length) results.winDetails = true;
      messages.push(`RoundResult: ${JSON.stringify(body.RoundResult)}`);
    }
    if ("CallRequired" in body) {
      // 响应窗口一律 Pass，避免游戏等待真人。
      commandId += 1;
      ws.send(JSON.stringify({
        protocol_version: 2,
        command_id: commandId,
        expected_seq: lastSeq,
        body: { CallResponse: { action: { Pass: null } } },
      }));
    }
    if ("ActionRequired" in body) {
      // 自动打出第一个合法弃牌，让对局持续进行。
      const request = body.ActionRequired;
      const tile = request.discard_options?.[0];
      if (tile !== undefined) {
        commandId += 1;
        ws.send(JSON.stringify({
          protocol_version: 2,
          command_id: commandId,
          expected_seq: lastSeq,
          body: { TurnAction: { action: { Discard: tile } } },
        }));
      }
    }
    messages.push(`seq=${message.seq} ${Object.keys(body)[0]}`);
  };

  await new Promise((resolve, reject) => {
    ws.onerror = (error) => reject(new Error(`WS error @ ${new Date().toISOString()}: ${error.message ?? String(error)}`));
    ws.onopen = () => {
      console.log(`✅ WS 已打开 @ ${new Date().toISOString()}`);
      setTimeout(resolve, (Number(process.env.WATCH_SECONDS) || 100) * 1000); // 观察，等待至少一局结算
    };
    ws.onclose = (event) => {
      console.log(`WS 关闭 code=${event.code} @ ${new Date().toISOString()}`);
      resolve();
    };
  });
  ws.close();

  console.log("结果摘要：", JSON.stringify(results));
  const handOk = results.handChecks > 0 && results.handErrors.length === 0;
  if (results.handErrors.length) console.log("牌数错误：", results.handErrors.slice(0, 5).join("; "));
  console.log(`收到消息 ${messages.length} 条，最后几条：`);
  console.log(messages.slice(-8).join("\n"));
  console.log(results.analysis > 0 && results.events > 0 && handOk ? "✅ 冒烟测试通过" : "❌ 关键数据缺失");
} catch (error) {
  console.error("❌ 冒烟测试失败：", error.message ?? error);
  if (error instanceof Error && error.stack) console.error(error.stack);
  console.error(log.split("\n").slice(-20).join("\n"));
  process.exitCode = 1;
} finally {
  server.kill();
}
