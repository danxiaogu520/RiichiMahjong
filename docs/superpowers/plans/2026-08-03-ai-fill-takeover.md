# AI 补位与断线托管 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为熟人房间增加房主可配置的固定基础 AI 补位，以及真人断线 30 秒后的 AI 临时托管和重连控制权恢复。

**Architecture:** 房间层把四个座位明确建模为真人、AI 补位或 AI 托管，并由房主通过 HTTP 配置 AI 数量。`riichi-session` 通过现有 agent 边界管理 AI 控制器；AI 与真人都只能向同一个 `GameSession` 提交命令，规则引擎保持唯一权威。浏览器通过房间视图和牌局事件显示 AI 状态，服务器重连/断线生命周期负责触发和取消 30 秒托管。

**Tech Stack:** Rust stable、Tokio、Axum WebSocket、Serde、现有 `riichi-core`/`riichi-logic`/`riichi-engine`/`riichi-ai`/`riichi-session` workspace、TypeScript、Vite。

## Global Constraints

- 房主可以配置 0–3 个 AI，且至少保留 1 名真人房主；支持 1 真人 + 3 AI、2 真人 + 2 AI、3 真人 + 1 AI 和 4 真人。
- 只有房主可以增加、移除或替换 AI 座位；未开局前朋友加入后由房主决定是否替换 AI。
- 首发只有一个固定基础 AI，不提供难度选择或多种 AI 策略。
- 真人断线后保留座位 30 秒，超过 30 秒由 AI 临时接管；真人重连后立即恢复控制。
- AI 在服务端运行，不访问浏览器隐藏信息，不直接修改 `GameState`；AI 命令必须经过同一套引擎合法性校验。
- 服务器重启不恢复内存房间；本计划不引入数据库、账号系统、公开匹配或手机专用 UI。
- AI 补位和 AI 托管状态必须在房间视图/牌桌状态中可见，并广播控制权变化。
- 所有自动化测试不得真实等待 30 秒；服务器必须支持测试注入更短的托管延迟。

---

## 文件结构与边界

实现前固定以下文件职责，避免把房间策略、AI 决策和网络展示混在同一个模块中：

- `crates/riichi-server/src/room.rs`：房主、座位类型、AI 数量、准备状态和开局前权限。
- `crates/riichi-server/src/application.rs`：房间门面、AI 初始 agent 注入、断线托管计时器和重连协调。
- `crates/riichi-server/src/transport.rs`：AI 配置 HTTP 路由、WebSocket 连接顺序和断线通知。
- `crates/riichi-session/src/channel.rs`：重连/安装 AI 的会话控制消息，以及 AI 状态事件。
- `crates/riichi-session/src/agent.rs`：通用 agent runner；agent 只产生命令，不访问 `GameState`。
- `crates/riichi-session/src/game.rs`：GameSession 内安装/取消 agent、替换玩家控制器和广播状态变化。
- `crates/riichi-ai/src/agent.rs`：固定基础 AI 的状态、牌效决策和 `PlayerAgent` 实现；从调试客户端抽出共享逻辑。
- `crates/riichi-ai/src/lib.rs`、`crates/riichi-ai/Cargo.toml`：导出基础 AI 并声明 session/Tokio 依赖。
- `crates/riichi-debug/src/main.rs`、`crates/riichi-debug/src/ai_client.rs`：改用共享基础 AI，删除重复决策实现。
- `crates/riichi-proto/src/messages.rs`、`crates/riichi-server/src/protocol.rs`：新增房间控制器变化的线协议及视图转换。
- `tauri-app/src/protocol.ts`、`tauri-app/src/transport.ts`、`tauri-app/src/main.ts`、`tauri-app/src/style.css`：房主 AI 设置、AI 标签、托管状态和控制权恢复反馈。
- `crates/riichi-server/tests/ai_room.rs`：至少一真人 + 三 AI 的端到端 session 验收。
- `docs/ONLINE_GAME_PLAN.md`、`README.md`：更新 AI 补位、断线托管和当前 MVP 边界。

---

### Task 1: 建模房间所有者、AI 座位与开局规则

**Files:**
- Modify: `crates/riichi-server/src/room.rs`
- Modify: `crates/riichi-server/src/application.rs`
- Test: `crates/riichi-server/src/room.rs` 的现有 `#[cfg(test)]` 模块
- Test: `crates/riichi-server/src/application.rs` 的现有 `#[cfg(test)]` 模块

**Interfaces:**
- `SeatController` 区分 `Human`、`AiFill` 和 `AiTakeover`。
- `Room` 新增 `owner: PlayerId`，保留四个稳定座位；AI 座位不拥有 token。
- `Room::set_ai_count(requester: PlayerId, ai_count: usize) -> Result<(), RoomError>` 只接受 0–3，并只允许 owner 调用。
- `Room::start(requester: PlayerId) -> Result<(), RoomError>` 要求 requester 是 owner、四个座位已填满、至少一个真人存在、所有真人已准备；AI 座位自动视为 ready。
- `Room::ai_players() -> Vec<PlayerId>` 返回 `AiFill` 或 `AiTakeover` 的座位。
- `ServerApplication::set_ai_count(room_id: &str, token: &str, ai_count: usize) -> Result<RoomInfo, RoomError>` 做 token 和 owner 校验后更新房间。
- `ServerApplication::launch_game(room_id: &str, token: &str) -> impl Future<Output = Result<RoomInfo, RoomError>>` 将开局权限校验集中在 application/room，而不是只在 HTTP handler 中校验。
- `RoomInfo` 增加 `owner: PlayerId`；`RoomPlayerView` 增加 `is_ai: bool` 和 `ai_takeover: bool`，由 Task 1 的 `room_info` 映射直接提供给后续 application/server 测试。

- [ ] **Step 1: 先写房间模型失败测试**

在 `room.rs` 的测试模块中补充以下行为测试；测试使用 `RoomManager::new`, `create_room`, `join` 和 `room_mut`，不通过 HTTP 绕过房间规则：

```rust
#[test]
fn owner_can_configure_zero_to_three_ai_seats() {
    let mut manager = RoomManager::new();
    let room_id = manager.create_room();
    let (owner, owner_token) = manager.join(&room_id, "房主").unwrap();

    manager
        .room_mut(&room_id)
        .unwrap()
        .set_ai_count(owner, 3)
        .unwrap();

    let room = manager.room(&room_id).unwrap();
    assert_eq!(room.owner, owner);
    assert_eq!(room.ai_players().len(), 3);
    assert_eq!(room.player_by_token(&owner_token).unwrap(), owner);
}

#[test]
fn non_owner_cannot_change_ai_count_or_start_room() {
    let mut manager = RoomManager::new();
    let room_id = manager.create_room();
    let (owner, _) = manager.join(&room_id, "房主").unwrap();
    let (guest, _) = manager.join(&room_id, "玩家").unwrap();

    assert_eq!(
        manager.room_mut(&room_id).unwrap().set_ai_count(guest, 2),
        Err(RoomError::NotRoomOwner)
    );
    assert_eq!(
        manager.room_mut(&room_id).unwrap().start(guest),
        Err(RoomError::NotRoomOwner)
    );
    assert_ne!(owner, guest);
}

#[test]
fn one_human_and_three_ai_can_start_after_human_ready() {
    let mut manager = RoomManager::new();
    let room_id = manager.create_room();
    let (owner, _) = manager.join(&room_id, "房主").unwrap();
    manager
        .room_mut(&room_id)
        .unwrap()
        .set_ai_count(owner, 3)
        .unwrap();
    manager
        .room_mut(&room_id)
        .unwrap()
        .set_ready(owner, true)
        .unwrap();

    manager.room_mut(&room_id).unwrap().start(owner).unwrap();
    assert!(manager.room(&room_id).unwrap().started);
}
```

- [ ] **Step 2: 运行房间测试确认当前实现失败**

Run: `cargo test -p riichi-server room::tests -- --nocapture`

Expected: FAIL because `Room` has no owner/AI controller model and `start` currently requires four human-ready seats.

- [ ] **Step 3: 增加明确的座位控制器和错误类型**

在 `room.rs` 中将当前 `RoomPlayer` 扩展为显式座位状态，避免用空 token 表示 AI：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeatController {
    Human,
    AiFill,
    AiTakeover,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomPlayer {
    pub id: PlayerId,
    pub nickname: String,
    pub token: Option<String>,
    pub ready: bool,
    pub connected: bool,
    pub controller: SeatController,
}
```

令第一个真人加入者成为 owner；AI 座位使用固定昵称 `AI`、`token: None`、`ready: true`、`connected: true`。token 查询、准备、连接和断开操作必须跳过 AI 座位。

- [ ] **Step 4: 实现 AI 数量配置和稳定座位分配**

`set_ai_count` 先移除已有 `AiFill` 座位，再在空座位中从北家到南家方向填充目标数量；不移动真人座位，不移除 `AiTakeover` 之外的真人。若目标 AI 数量与真人数量无法组成四席，返回 `InvalidAiCount`。AI 数量变化只允许在 `started == false` 时进行。

- [ ] **Step 5: 实现 owner 开局校验并更新 application 门面**

将 `Room::start()` 改为 `Room::start(requester)`，由 `ServerApplication::launch_game` 共用 owner/token 校验。同步更新 `room_info` 映射，输出 `owner`, `is_ai` 和 `ai_takeover`，不输出任何 token。

- [ ] **Step 6: 运行房间和 application 测试确认通过**

Run: `cargo test -p riichi-server room::tests -- --nocapture`

Run: `cargo test -p riichi-server application::tests -- --nocapture`

Expected: PASS，且原有 token 校验、房间清理测试继续通过。

- [ ] **Step 7: 提交房间模型变更**

```bash
git add crates/riichi-server/src/room.rs crates/riichi-server/src/application.rs
git commit -m "feat: model AI seats and room ownership"
```

### Task 2: 抽取固定基础 AI 并建立通用 agent runner

**Files:**
- Modify: `crates/riichi-session/src/agent.rs`
- Modify: `crates/riichi-session/src/channel.rs`
- Modify: `crates/riichi-session/src/lib.rs`
- Modify: `crates/riichi-ai/Cargo.toml`
- Modify: `crates/riichi-ai/src/lib.rs`
- Create: `crates/riichi-ai/src/agent.rs`
- Modify: `crates/riichi-debug/src/main.rs`
- Delete: `crates/riichi-debug/src/ai_client.rs`
- Test: `crates/riichi-ai/src/agent.rs` 的测试模块

**Interfaces:**
- `PlayerAgent::decide` 返回 `Option<PlayerAction>`；普通状态事件返回 `None`，只在行动事件上返回命令。
- `pub async fn run_player_agent(event_rx: mpsc::Receiver<SessionEvent>, action_tx: mpsc::Sender<PlayerCommand>, agent: Box<dyn PlayerAgent>)` 负责把 agent 输出包装成带座位的 `PlayerCommand`。
- `pub struct BasicAiAgent` 实现 `PlayerAgent`，构造函数为 `BasicAiAgent::new(player: PlayerId)`。
- `BasicAiAgent` 复用现有 `choose_discard`, `decide_riichi`, `decide_call`；基础策略不新增难度参数。
- `SessionControl` 改为可表达 `Reconnect` 和 `InstallAgent` 两种控制消息，具体定义在 Task 3 完成。

- [ ] **Step 1: 先为 agent runner 写失败测试**

在 `riichi-session/src/agent.rs` 测试中定义一个只对 `ActionRequired` 返回 `Discard`、对其他事件返回 `None` 的测试 agent，并验证 runner 不会把无关事件误发成命令：

```rust
#[tokio::test]
async fn runner_only_forwards_agent_actions() {
    let (event_tx, event_rx) = mpsc::channel(4);
    let (action_tx, mut action_rx) = mpsc::channel(4);
    let agent = Box::new(TestAgent::new(PlayerId(1), Tile::from_raw(0)));

    tokio::spawn(run_player_agent(event_rx, action_tx, agent));
    event_tx.send(SessionEvent::Error("无关事件".to_string())).await.unwrap();
    assert!(tokio::time::timeout(Duration::from_millis(10), action_rx.recv()).await.is_err());

    event_tx.send(SessionEvent::ActionRequired {
        can_tsumo: false,
        can_riichi: false,
        riichi_options: Vec::new(),
        discard_options: vec![Tile::from_raw(0)],
        ankan_options: Vec::new(),
        kakan_options: Vec::new(),
        can_kyuushu: false,
    }).await.unwrap();
    let command = tokio::time::timeout(Duration::from_millis(100), action_rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(command.player, PlayerId(1));
    assert!(matches!(command.action, PlayerAction::TurnAction(TurnAction::Discard(_))));
}

struct TestAgent {
    player: PlayerId,
    tile: Tile,
}

impl TestAgent {
    fn new(player: PlayerId, tile: Tile) -> Self {
        Self { player, tile }
    }
}

impl PlayerAgent for TestAgent {
    fn player_id(&self) -> PlayerId {
        self.player
    }

    fn decide<'a>(&'a mut self, observation: SessionEvent) -> AgentFuture<'a> {
        let tile = self.tile;
        Box::pin(async move {
            match observation {
                SessionEvent::ActionRequired { .. } => Some(PlayerAction::TurnAction(TurnAction::Discard(tile))),
                _ => None,
            }
        })
    }
}
```

- [ ] **Step 2: 运行 session agent 测试确认接口不匹配**

Run: `cargo test -p riichi-session agent::tests::runner_only_forwards_agent_actions -- --nocapture`

Expected: FAIL until `PlayerAgent` 支持无动作事件并且 runner 存在。

- [ ] **Step 3: 实现 `PlayerAgent` 的可选命令和 runner**

在 `agent.rs` 中将接口固定为：

```rust
pub type AgentFuture<'a> = Pin<Box<dyn Future<Output = Option<PlayerAction>> + Send + 'a>>;

pub trait PlayerAgent: Send + 'static {
    fn player_id(&self) -> PlayerId;
    fn decide<'a>(&'a mut self, observation: SessionEvent) -> AgentFuture<'a>;
}

pub async fn run_player_agent(
    mut event_rx: mpsc::Receiver<SessionEvent>,
    action_tx: mpsc::Sender<PlayerCommand>,
    mut agent: Box<dyn PlayerAgent>,
) {
    while let Some(event) = event_rx.recv().await {
        let player = agent.player_id();
        if let Some(action) = agent.decide(event).await {
            if action_tx.send(PlayerCommand::new(player, action)).await.is_err() {
                break;
            }
        }
    }
}
```

- [ ] **Step 4: 将当前调试 AI 逻辑抽到 `riichi-ai/src/agent.rs`**

把 `crates/riichi-debug/src/ai_client.rs` 中的 `AiState`, `build_visible_tiles`, `decide_turn` 和响应处理移入 `BasicAiAgent`。`StateUpdate` 更新内部手牌、阶段和可见牌；`ActionRequired` 优先自摸、再选择合法立直牌、最后选择牌效最好的合法舍牌；`CallRequired` 有荣和时荣和，否则 Pass。所有 fallback 必须从服务端提供的候选列表中选择。

在 `riichi-ai/Cargo.toml` 增加：

```toml
riichi-session = { path = "../riichi-session" }
tokio = { version = "1", features = ["time"] }
```

从 `riichi-ai/src/lib.rs` 导出 `BasicAiAgent`；debug 客户端改为调用 `run_player_agent` 和 `BasicAiAgent::new`，不再保留第二份 AI 决策逻辑。

- [ ] **Step 5: 为基础 AI 写决策测试**

至少覆盖以下函数/行为：

```rust
#[tokio::test]
async fn basic_ai_returns_a_legal_discard_from_action_options() {
    let mut ai = BasicAiAgent::new(PlayerId(0));
    let action = ai.decide(SessionEvent::ActionRequired {
        can_tsumo: false,
        can_riichi: false,
        riichi_options: Vec::new(),
        discard_options: vec![Tile::from_raw(0)],
        ankan_options: Vec::new(),
        kakan_options: Vec::new(),
        can_kyuushu: false,
    }).await;
    assert!(matches!(action, Some(PlayerAction::TurnAction(TurnAction::Discard(tile))) if tile == Tile::from_raw(0)));
}

#[tokio::test]
async fn basic_ai_ron_or_passes_call_options() {
    let mut ai = BasicAiAgent::new(PlayerId(0));
    let action = ai.decide(SessionEvent::CallRequired {
        options: vec![CallOption { player: PlayerId(0), call_type: CallType::Ron }],
    }).await;
    assert!(matches!(action, Some(PlayerAction::CallResponse(CallResponse::Ron))));

    let action = ai.decide(SessionEvent::CallRequired {
        options: vec![CallOption {
            player: PlayerId(0),
            call_type: CallType::Pon { hand_tiles: [Tile::from_raw(0), Tile::from_raw(1)] },
        }],
    }).await;
    assert!(matches!(action, Some(PlayerAction::CallResponse(CallResponse::Pass))));
}
```

测试模块需要导入 `CallOption`, `CallType`, `CallResponse`, `PlayerAction`, `SessionEvent`, `Tile` 和 `PlayerId`，只构造最小合法 `SessionEvent`，不启动完整牌局；完整牌局在 Task 6 验证。

- [ ] **Step 6: 运行 AI 与 debug 测试**

Run: `cargo test -p riichi-ai`

Run: `cargo test -p riichi-debug`

Expected: PASS，且现有 AI 牌效测试数量不减少。

- [ ] **Step 7: 提交共享基础 AI**

```bash
git add crates/riichi-session/src/agent.rs crates/riichi-session/src/lib.rs crates/riichi-ai/Cargo.toml crates/riichi-ai/src/lib.rs crates/riichi-ai/src/agent.rs crates/riichi-debug/src/main.rs crates/riichi-debug/src/ai_client.rs
git commit -m "refactor: share basic AI through session agents"
```

### Task 3: 让 GameSession 支持初始 AI 与控制器切换

**Files:**
- Modify: `crates/riichi-session/src/channel.rs`
- Modify: `crates/riichi-session/src/game.rs`
- Modify: `crates/riichi-session/src/lib.rs`
- Test: `crates/riichi-session/src/game.rs` 的测试模块

**Interfaces:**
- `SessionControl` 变为：

```rust
pub enum SessionControl {
    Reconnect {
        player: PlayerId,
        last_event_id: u64,
        event_tx: mpsc::Sender<SessionEvent>,
        action_rx: mpsc::Receiver<PlayerCommand>,
    },
    InstallAgent {
        player: PlayerId,
        agent: Box<dyn PlayerAgent>,
    },
}
```

- 增加 `SessionEvent::PlayerControllerChanged { player: PlayerId, is_ai: bool, ai_takeover: bool }`。
- 增加 `GameSession::new_with_control_and_agents(..., initial_agents: Vec<(PlayerId, Box<dyn PlayerAgent>)>) -> Self`；保留现有 `new`/`new_with_control` 包装器，默认 agent 列表为空，保证终端和旧测试可以渐进迁移。
- `GameSession` 增加 `agent_tasks: [Option<JoinHandle<()>>; 4]`，安装新控制器前 abort 旧 agent task。
- `GameSession::install_agent(player, agent)` 创建该座位专属事件 channel，启动 `run_player_agent`，将事件 sender 放入 `event_txs[player]`，然后广播控制器变化和最新状态。
- `GameSession::reconnect_player(...)` 在替换真人通道前 abort 对应 agent task，并广播 `{ is_ai: false, ai_takeover: false }`。

- [ ] **Step 1: 写 agent 安装/抢回控制权失败测试**

在 `game.rs` 测试模块新增一个 `TestAgent`，它收到 `ActionRequired` 时立即发送 `Discard`，然后验证安装 agent 后能看到控制器事件，重连后 agent task 被取消且只接收真人通道动作：

```rust
#[tokio::test]
async fn install_agent_broadcasts_controller_change_and_reconnect_restores_human() {
    let (event_tx, mut event_rx) = mpsc::channel(32);
    let event_txs = [event_tx.clone(), event_tx.clone(), event_tx.clone(), event_tx];
    let (action_tx, action_rx) = mpsc::channel(32);
    let (_, control_rx) = mpsc::channel(8);
    let mut session = GameSession::new_with_control(event_txs, action_tx, action_rx, control_rx);

    session
        .install_agent(PlayerId(0), Box::new(TestAgent::new(PlayerId(0))))
        .await;

    assert!(matches!(
        event_rx.recv().await,
        Some(SessionEvent::PlayerControllerChanged { player: PlayerId(0), is_ai: true, ai_takeover: true })
    ));

    let (replacement_tx, mut replacement_rx) = mpsc::channel(8);
    let (_, replacement_action_rx) = mpsc::channel(8);
    session
        .reconnect_player(PlayerId(0), 0, replacement_tx, replacement_action_rx)
        .await;
    assert!(matches!(
        replacement_rx.recv().await,
        Some(SessionEvent::PlayerControllerChanged { player: PlayerId(0), is_ai: false, ai_takeover: false })
    ));
}
```

- [ ] **Step 2: 运行 session 测试确认缺少控制器切换**

Run: `cargo test -p riichi-session game::tests::install_agent_broadcasts_controller_change_and_reconnect_restores_human -- --nocapture`

Expected: FAIL until `SessionControl`, agent task 和控制器事件存在。

- [ ] **Step 3: 实现 agent task 生命周期**

在 `GameSession::run` 开始 `start_round` 前安装 `initial_agents`。在行动等待和响应等待中统一处理两类 control：`Reconnect` 调用现有重连逻辑，`InstallAgent` 调用 `install_agent` 后继续等待当前阶段，不重置牌局状态。所有 agent 命令仍进入同一个 `self.action_rx`，之后由 `validate_action`/响应优先级逻辑处理。

- [ ] **Step 4: 广播控制器变化并保护隐藏信息**

`PlayerControllerChanged` 不包含手牌、摸牌或其他隐藏字段，只包含座位编号和控制器状态。`broadcast` 给四个事件队列；重连客户端通过现有快照流程恢复完整玩家视角，再收到当前控制器状态。

- [ ] **Step 5: 运行 session 全量测试**

Run: `cargo test -p riichi-session`

Expected: PASS，包括原有响应优先级和重连测试，以及新增 agent 安装/真人抢回测试。

- [ ] **Step 6: 提交 GameSession 控制器切换**

```bash
git add crates/riichi-session/src/channel.rs crates/riichi-session/src/agent.rs crates/riichi-session/src/game.rs crates/riichi-session/src/lib.rs
git commit -m "feat: switch game session controllers between humans and AI"
```

### Task 4: 接入公网房间的 AI 开局和 30 秒断线托管

**Files:**
- Modify: `crates/riichi-server/Cargo.toml`
- Modify: `crates/riichi-server/src/application.rs`
- Modify: `crates/riichi-server/src/transport.rs`
- Modify: `crates/riichi-server/src/room.rs`
- Test: `crates/riichi-server/src/application.rs` 的测试模块
- Test: `crates/riichi-server/src/transport.rs` 的测试模块

**Interfaces:**
- `ActiveSession` 保存 `control_tx`；`ServerApplication` 额外保存 `ai_takeover_delay: Duration`，默认值为 30 秒，并提供 `new_with_ai_takeover_delay(delay: Duration) -> Self` 供测试使用。
- `ServerApplication::launch_game(room_id: &str, token: &str) -> impl Future<Output = Result<RoomInfo, RoomError>>` 从 `Room::ai_players()` 创建 `BasicAiAgent`，完成 owner 校验、房间 start 和 `GameSession::new_with_control_and_agents`。
- `ServerApplication::disconnect_player` 标记真人断线并记录递增 `connection_generation`，仅在已有 active session 时启动托管任务；它返回 `impl Future<Output = Result<PlayerId, RoomError>>`。
- `ServerApplication::connect_player` 在 token 重连时恢复真人控制状态；`websocket` 必须在建立 session channels 前调用它，确保 AI 托管座位先被标记为真人恢复。
- `ServerApplication::room_info(room_id: &str) -> Result<RoomInfo, RoomError>` 仅用于测试和内部状态读取，不返回 token。
- `ServerApplication::can_take_over(room_id: &str, player: PlayerId, generation: u64) -> bool` 检查延迟任务是否仍然有效。
- `ServerApplication::install_ai_takeover(room_id: &str, player: PlayerId, generation: u64) -> impl Future<Output = Result<(), RoomError>>` 标记房间状态并向 active session 发送 `InstallAgent`。
- `pub type SessionEventReceiver = Arc<Mutex<mpsc::Receiver<SessionEvent>>>` 从 `application.rs` 导出，供 server integration test 消费玩家视角事件。
- 新 HTTP 请求和路由：

```rust
#[derive(Debug, Deserialize)]
pub struct AiRequest {
    pub token: String,
    pub count: usize,
}

// POST /rooms/:room_id/ai
// body: { "token": "...", "count": 0..3 }
```

- [ ] **Step 1: 为 application 写权限和初始 AI 失败测试**

在 application 测试中覆盖 owner、AI 数量和初始 session：

```rust
#[tokio::test]
async fn owner_can_start_one_human_three_ai_game() {
    let app = ServerApplication::new();
    let room = app.create_room();
    let owner = app.join_room(&room.id, "房主").unwrap();

    app.set_ai_count(&room.id, &owner.token, 3).unwrap();
    app.set_ready(&room.id, &owner.token, true).unwrap();
    let started = app.launch_game(&room.id, &owner.token).await.unwrap();

    assert!(started.started);
    assert_eq!(started.players.iter().filter(|player| player.is_ai).count(), 3);
    app.finish_game(&room.id).await.unwrap();
}

#[tokio::test]
async fn disconnected_human_is_taken_over_only_after_injected_delay() {
    let app = ServerApplication::new_with_ai_takeover_delay(Duration::from_millis(10));
    let room = app.create_room();
    let owner = app.join_room(&room.id, "房主").unwrap();
    app.set_ai_count(&room.id, &owner.token, 3).unwrap();
    app.set_ready(&room.id, &owner.token, true).unwrap();
    app.launch_game(&room.id, &owner.token).await.unwrap();

    app.disconnect_player(&room.id, &owner.token).await.unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;
    let room_after = app.room_info(&room.id).unwrap();
    assert!(room_after.players.iter().any(|player| player.ai_takeover));
    app.finish_game(&room.id).await.unwrap();
}
```

- [ ] **Step 2: 运行 application 测试确认新 API 尚不存在**

Run: `cargo test -p riichi-server application::tests::owner_can_start_one_human_three_ai_game -- --nocapture`

Expected: FAIL because `RoomInfo` has no AI view, `set_ai_count` and async disconnect takeover do not exist.

- [ ] **Step 3: 接入 `riichi-ai` 和 initial agents**

在 `crates/riichi-server/Cargo.toml` 增加：

```toml
riichi-ai = { path = "../riichi-ai" }
```

`launch_game` 读取 room 的 AI 座位，构造：

```rust
let initial_agents = room.ai_players()
    .into_iter()
    .map(|player| (player, Box::new(BasicAiAgent::new(player)) as Box<dyn PlayerAgent>))
    .collect();
```

把 `initial_agents` 传入 Task 3 的 `new_with_control_and_agents`。人类座位继续由 WebSocket `SessionControl::Reconnect` 注入。

- [ ] **Step 4: 实现 30 秒 generation-checked 托管计时器**

在 `Room` 为每个真人座位维护 `connection_generation`；断开时递增并返回 generation。延迟任务醒来后必须同时满足以下条件才安装 AI：同一房间仍存在、同一座位 generation 未变化、座位仍为 Human 且 disconnected、active session 仍存在。安装成功后将房间座位标记为 `AiTakeover`，向 session 发送 `SessionControl::InstallAgent`。

伪代码必须对应以下实际流程，不允许无条件 sleep 后接管：

```rust
let generation = room.mark_disconnected(player)?;
let application = self.clone();
tokio::spawn(async move {
    tokio::time::sleep(application.ai_takeover_delay).await;
    if application.can_take_over(room_id, player, generation) {
        application.install_ai_takeover(room_id, player, generation).await;
    }
});
```

重连路径先调用 `Room::reconnect_by_token`，若座位是 `AiTakeover` 则改回 `Human`、`connected = true`，再向 GameSession 发送 `Reconnect`，由 GameSession abort agent task 并发送最新提示。

- [ ] **Step 5: 增加 AI 配置 HTTP endpoint 并收紧 start 权限**

在 `transport.rs` 注册：

```rust
Router::new()
    .route("/rooms/:room_id/ai", post(set_ai_count))
```

handler 调用 `application.set_ai_count(room_id, &request.token, request.count)`。`start_room` handler 不再先 authenticate 再无 token 调用 launch；直接调用 `application.launch_game(room_id, &request.token).await`，确保只有房主能开始。

- [ ] **Step 6: 运行 server 测试和格式检查**

Run: `cargo fmt --all -- --check`

Run: `cargo test -p riichi-server`

Expected: PASS；HTTP 测试覆盖 AI endpoint、非 owner 拒绝、非法 count、owner start 和 30 秒计时器 generation race。

- [ ] **Step 7: 提交服务端 AI 生命周期**

```bash
git add crates/riichi-server/Cargo.toml crates/riichi-server/src/room.rs crates/riichi-server/src/application.rs crates/riichi-server/src/transport.rs
git commit -m "feat: add server AI fill and disconnect takeover"
```

### Task 5: 扩展协议并完成桌面浏览器房主设置与状态反馈

**Files:**
- Modify: `crates/riichi-server/src/application.rs`
- Modify: `crates/riichi-server/src/protocol.rs`
- Modify: `crates/riichi-proto/src/messages.rs`
- Modify: `tauri-app/src/protocol.ts`
- Modify: `tauri-app/src/transport.ts`
- Modify: `tauri-app/src/main.ts`
- Modify: `tauri-app/src/style.css`
- Test: `crates/riichi-server/src/protocol.rs` 的测试模块

**Interfaces:**
- `RoomInfo` 增加 `owner: PlayerId`。
- `RoomPlayerView` 增加 `is_ai: bool` 和 `ai_takeover: bool`；AI 补位为 `is_ai=true, ai_takeover=false`，断线托管为 `is_ai=true, ai_takeover=true`。
- `ServerMessage` 增加：

```rust
PlayerControllerChanged {
    player_id: PlayerId,
    is_ai: bool,
    ai_takeover: bool,
},
```

- `ClientTransport.setAiCount(roomId: string, token: string, count: number): Promise<RoomInfo>` 调用 `POST /rooms/:room_id/ai`。
- TypeScript 类型与 Rust JSON 字段保持 snake_case；不在 WebSocket 中新增玩家身份字段。

- [ ] **Step 1: 写 wire view 和控制器事件失败测试**

在 `protocol.rs` 添加：

```rust
#[test]
fn controller_change_event_is_serialized_without_hidden_game_state() {
    let event = SessionEvent::PlayerControllerChanged {
        player: PlayerId(2),
        is_ai: true,
        ai_takeover: true,
    };
    let message = session_event_to_wire(&event, PlayerId(0)).unwrap();
    assert!(matches!(
        message,
        ServerMessage::PlayerControllerChanged {
            player_id: PlayerId(2),
            is_ai: true,
            ai_takeover: true
        }
    ));
    let encoded = serde_json::to_string(&message).unwrap();
    assert!(!encoded.contains("hand"));
}
```

- [ ] **Step 2: 运行协议测试确认缺失 wire variant**

Run: `cargo test -p riichi-server protocol::tests::controller_change_event_is_serialized_without_hidden_game_state -- --nocapture`

Expected: FAIL until proto/server conversion支持控制器事件。

- [ ] **Step 3: 增加 Rust proto 和 server 转换**

在 `riichi-proto::ServerMessage`、`SessionEvent` 转换和 `state_snapshot_to_wire` 相关匹配分支中处理新事件。控制器事件不改变 `seq` 以外的牌局状态，不复制 `GameStateView`。

- [ ] **Step 4: 增加前端房主 AI 设置**

在 `protocol.ts` 增加：

```ts
export interface RoomPlayerView {
  id: PlayerId;
  nickname: string;
  ready: boolean;
  connected: boolean;
  is_ai: boolean;
  ai_takeover: boolean;
}

export interface RoomInfo {
  id: string;
  owner: PlayerId;
  players: RoomPlayerView[];
  started: boolean;
}
```

在 `renderLobby` 中只向 `session.player === room.owner` 的用户显示 AI 数量控制。当前数量由 `room.players.filter(player => player.is_ai).length` 得出，范围 0–3；调用 `setAiCount` 后使用服务端返回的 `RoomInfo` 重新渲染，不在前端本地猜测座位分配。

- [ ] **Step 5: 增加 AI/托管状态展示**

`renderPlayers` 为 AI 补位显示 `AI`，为 `ai_takeover` 显示 `AI 托管`；真人断线显示 `等待重连`。`handleServerMessage` 收到 `PlayerControllerChanged` 时更新对应 `room.players`，并在 `statusMessage` 中显示“AI 已接管”或“已恢复真人控制”。牌桌四家座位复用同一状态字段。

- [ ] **Step 6: 调整 lobby 开始按钮条件**

开始按钮必须同时满足：当前用户是 owner、`room.players.length === 4`、至少一个真人、所有真人 `ready === true`。AI 不需要点击准备。服务端仍是最终校验，前端条件只负责减少无效请求。

- [ ] **Step 7: 运行协议、TypeScript 和前端构建测试**

Run: `cargo test -p riichi-proto -p riichi-server`

Run: `cd tauri-app && npm run build`

Expected: PASS；浏览器构建产物包含 AI 设置、AI 标记和托管状态逻辑。

- [ ] **Step 8: 提交协议和浏览器体验**

```bash
git add crates/riichi-proto/src/messages.rs crates/riichi-server/src/application.rs crates/riichi-server/src/protocol.rs tauri-app/src/protocol.ts tauri-app/src/transport.ts tauri-app/src/main.ts tauri-app/src/style.css
git commit -m "feat: expose AI room controls and takeover status"
```

### Task 6: 建立 AI 房间端到端验收与回归覆盖

**Files:**
- Create: `crates/riichi-server/tests/ai_room.rs`
- Modify: `crates/riichi-engine/tests/half_game.rs`（若现有测试辅助可复用则只扩展，不重写）
- Modify: `crates/riichi-server/src/application.rs` 测试
- Modify: `crates/riichi-session/src/game.rs` 测试
- Modify: `docs/ONLINE_GAME_PLAN.md`
- Modify: `README.md`

**Interfaces:**
- 新的 server integration test 使用 `ServerApplication`、`session_channels` 和一个确定性真人 driver；不依赖公网、浏览器或真实 30 秒等待。
- driver 在收到 `ActionRequired` 时优先提交 `Tsumo`，否则提交 `discard_options[0]`；收到 `CallRequired` 时提交 `Pass`。每次命令都只使用当前事件提供的候选动作。
- 测试超时使用 `ServerApplication::new_with_ai_takeover_delay(Duration::from_millis(10))`。

- [ ] **Step 1: 写一真人三 AI 的失败集成测试**

创建 `crates/riichi-server/tests/ai_room.rs`，先加入以下 imports，再测试 session 收到 AI 和真人命令后能正常发送 `GameOver`：

```rust
use std::time::Duration;
use tokio::sync::mpsc;
use riichi_core::player::PlayerId;
use riichi_session::{CallResponse, PlayerAction, PlayerCommand, SessionEvent, TurnAction};
use riichi_server::application::{ServerApplication, SessionEventReceiver};

#[tokio::test]
async fn one_human_three_ai_room_reaches_game_over() {
    let app = ServerApplication::new_with_ai_takeover_delay(Duration::from_millis(10));
    let room = app.create_room();
    let owner = app.join_room(&room.id, "房主").unwrap();
    app.set_ai_count(&room.id, &owner.token, 3).unwrap();
    app.set_ready(&room.id, &owner.token, true).unwrap();
    app.launch_game(&room.id, &owner.token).await.unwrap();

    let (action_tx, event_rx) = app
        .session_channels(&room.id, owner.player, 0)
        .await
        .unwrap();
    let human = tokio::spawn(drive_human_player(owner.player, action_tx, event_rx));
    tokio::time::timeout(Duration::from_secs(5), human)
        .await
        .expect("AI room should finish within five seconds")
        .unwrap()
        .unwrap();
}
```

- [ ] **Step 2: 运行集成测试确认缺少 driver 或 AI wiring**

Run: `cargo test -p riichi-server --test ai_room -- --nocapture`

Expected: FAIL until Tasks 2–5 的 agent、room、session 和 application wiring 完成。

- [ ] **Step 3: 实现确定性真人 driver**

在测试文件中实现：

```rust
async fn drive_human_player(
    player: PlayerId,
    action_tx: mpsc::Sender<PlayerCommand>,
    mut event_rx: SessionEventReceiver,
) -> Result<(), String> {
    while let Some(event) = {
        let mut receiver = event_rx.lock().await;
        receiver.recv().await
    } {
        match event {
            SessionEvent::ActionRequired { discard_options, can_tsumo, .. } => {
                let action = if can_tsumo {
                    TurnAction::Tsumo
                } else {
                    TurnAction::Discard(discard_options.first().copied().ok_or("no discard")?)
                };
                action_tx
                    .send(PlayerCommand::new(player, PlayerAction::TurnAction(action)))
                    .await
                    .map_err(|error| error.to_string())?;
            }
            SessionEvent::CallRequired { .. } => {
                action_tx
                    .send(PlayerCommand::new(
                        player,
                        PlayerAction::CallResponse(CallResponse::Pass),
                    ))
                    .await
                    .map_err(|error| error.to_string())?;
            }
            SessionEvent::GameOver { .. } => return Ok(()),
            _ => {}
        }
    }
    Err("session ended before GameOver".to_string())
}
```

`SessionEventReceiver` 必须由 application 导出，集成测试直接复用该类型，不复制 `Arc<Mutex<Receiver<_>>>` 的内部定义。

- [ ] **Step 4: 增加断线托管和真人抢回的集成覆盖**

在 `ai_room.rs` 中增加两个短延迟测试，完整断言 generation 校验和控制器事件：

```rust
#[tokio::test]
async fn reconnect_before_delay_prevents_ai_takeover() {
    let app = ServerApplication::new_with_ai_takeover_delay(Duration::from_millis(20));
    let room = app.create_room();
    let owner = app.join_room(&room.id, "房主").unwrap();
    app.set_ai_count(&room.id, &owner.token, 3).unwrap();
    app.set_ready(&room.id, &owner.token, true).unwrap();
    app.launch_game(&room.id, &owner.token).await.unwrap();

    app.disconnect_player(&room.id, &owner.token).await.unwrap();
    tokio::time::sleep(Duration::from_millis(5)).await;
    app.connect_player(&room.id, &owner.token).unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;

    let room_after = app.room_info(&room.id).unwrap();
    let owner_view = room_after.players.iter().find(|player| player.id == owner.player).unwrap();
    assert!(!owner_view.ai_takeover);
    assert!(!owner_view.is_ai);
    app.finish_game(&room.id).await.unwrap();
}

#[tokio::test]
async fn reconnect_after_takeover_restores_human_controller() {
    let app = ServerApplication::new_with_ai_takeover_delay(Duration::from_millis(10));
    let room = app.create_room();
    let owner = app.join_room(&room.id, "房主").unwrap();
    app.set_ai_count(&room.id, &owner.token, 3).unwrap();
    app.set_ready(&room.id, &owner.token, true).unwrap();
    app.launch_game(&room.id, &owner.token).await.unwrap();

    app.disconnect_player(&room.id, &owner.token).await.unwrap();
    tokio::time::sleep(Duration::from_millis(25)).await;
    app.connect_player(&room.id, &owner.token).unwrap();
    let (_, event_rx) = app.session_channels(&room.id, owner.player, 0).await.unwrap();

    let mut restored = false;
    for _ in 0..12 {
        if let Some(SessionEvent::PlayerControllerChanged { player, is_ai, ai_takeover }) =
            tokio::time::timeout(Duration::from_millis(100), event_rx.lock().await.recv()).await.unwrap()
        {
            if player == owner.player && !is_ai && !ai_takeover {
                restored = true;
                break;
            }
        }
    }
    assert!(restored);
    app.finish_game(&room.id).await.unwrap();
}
```

测试必须通过 generation 校验验证旧计时器不会在玩家已经重连后安装 AI，并通过事件队列验证 `AiTakeover -> Human` 的顺序。`session_channels` 的返回 receiver 使用 application 导出的 `SessionEventReceiver` 类型。

- [ ] **Step 5: 更新项目文档**

在 `README.md` 快速开始或能力说明中写明：熟人房间支持 0–3 个固定基础 AI，至少一名真人可以开局，真人断线 30 秒后 AI 临时接管。更新 `docs/ONLINE_GAME_PLAN.md` 当前进度和 MVP 范围，明确 AI 不提供难度选择、房间仍为内存状态。

- [ ] **Step 6: 运行最终验证命令**

Run: `cargo fmt --all -- --check`

Run: `cargo test --workspace`

Run: `cd tauri-app && npm run build`

Expected: 三条命令全部 PASS；workspace 测试包含房间权限、agent runner、控制器切换、AI 房间完成和协议视角测试。

- [ ] **Step 7: 提交端到端覆盖和文档**

```bash
git add crates/riichi-server/tests/ai_room.rs crates/riichi-engine/tests/half_game.rs crates/riichi-server/src/application.rs crates/riichi-session/src/game.rs docs/ONLINE_GAME_PLAN.md README.md
git commit -m "test: verify AI room and takeover flow"
```

## Self-Review Checklist

- [x] 规格中的房主权限、0–3 AI、至少一真人开局、AI 补位、30 秒托管、重连抢回和可见状态均有对应任务。
- [x] AI 决策与 GameState 解耦，所有命令仍经过 `GameSession` 和 `riichi-engine`。
- [x] 不会在测试中真实等待 30 秒；所有托管测试使用注入延迟。
- [x] 没有引入数据库、账号、匹配、手机 UI 或 AI 难度系统。
- [x] 所有任务都有明确文件、接口、失败测试、通过命令和独立提交点。
- [x] 最终验证覆盖 Rust workspace、前端生产构建和至少一真人三 AI 的完整对局。
