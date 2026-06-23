# 架构文档

## Crate 职责

### riichi-core（纯数据结构）

不包含任何游戏逻辑，只定义数据类型。

| 模块 | 内容 |
|------|------|
| `tile.rs` | `Tile(u8)`, `TileType(u8)`, `Suit`, `Rank` |
| `hand.rs` | `Hand` — 手牌（排序的 `Vec<Tile>`） |
| `meld.rs` | `Meld`, `MeldKind` — 副露（吃/碰/杠） |
| `wall.rs` | `Wall` — 牌山（136 张牌的洗牌/摸牌/杠管理） |
| `player.rs` | `PlayerId`, `wind_from_index`, `next_wind`, `wind_display` |
| `player_state.rs` | `Player` — 玩家状态（手牌/分数/副露/立直/振听，一发/食替/双立直等通过事件查询） |
| `game_types.rs` | `GamePhase`, `GameEvent`, `TurnAction`, `ResponseAction`, `CallOption`, `GameError` |

### riichi-logic（纯算法）

所有计算逻辑，不依赖 `rand`。

| 模块 | 内容 |
|------|------|
| `shanten.rs` | `ShantenCalculator` — 向听数计算（查表法） |
| `acceptance.rs` | `analyze_discard`, `analyze_acceptance` — 打牌分析/进张/改良 |
| `win_check.rs` | `check_win` — 和了判定入口 |
| `analysis.rs` | `is_winning`, `decompose_hand`, `analyze_wait_tiles` — 手牌分解/听牌分析 |
| `fu.rs` | `calculate_fu` — 符数计算 |
| `scoring.rs` | `calculate_points` — 点数计算 |
| `dora.rs` | `calculate_dora` — 宝牌计算 |
| `types.rs` | `YakuName`, `WinContext`, `TileCounts`, `WinResult` 等 |

### riichi-ai（决策层）

使用 riichi-logic 的算法做出决策。

| 模块 | 内容 |
|------|------|
| `discard.rs` | `choose_discard` — AI 打牌选择 |
| `call_decision.rs` | `decide_call` — AI 副露决策（当前：一律 Pass） |
| `riichi_decision.rs` | `decide_riichi` — AI 立直决策 |

### riichi-engine（状态机）

游戏状态管理，回合流程控制。仅依赖 riichi-core + riichi-logic，不依赖 riichi-ai。

| 模块 | 内容 |
|------|------|
| `game.rs` | `GameState` 结构体定义 |
| `init.rs` | 初始化、庄家、杠数、宝牌 |
| `round.rs` | 配牌、摸牌、打牌 |
| `action.rs` | 行动执行（自摸/立直/暗杠/加杠/响应） |
| `riichi.rs` | 立直相关（听牌检查、立直宣言、立直后暗杠） |
| `win.rs` | 和了判定、计分 |
| `state.rs` | 游戏状态辅助（回合推进、振听、可见牌） |
| `query.rs` | 事件查询（副露/第一巡/立直数） |
| `abort.rs` | 流局检测（九种九牌/四风连打/四家立直/四杠散了） |
| `settlement.rs` | 结算（荒牌罚符、连庄/过庄） |
| `call.rs` | 副露检测（吃/碰/杠/荣和） |

### riichi-proto（通信协议）

定义客户端-服务端消息格式，使用 serde 序列化。

| 类型 | 说明 |
|------|------|
| `ClientMessage` | 客户端→服务端：行动/副露响应 |
| `ServerMessage` | 服务端→客户端：状态更新/事件/请求 |
| `GameStateView` | 玩家视角（隐藏他人手牌） |
| `AnalysisInfo` | 服务端计算的分析信息 |

### riichi-server（服务端）

游戏实例管理、AI 控制、CLI 显示。

### riichi-client（客户端）

ratatui 终端 UI，纯展示层。

## 牌编码

```
Tile(u8): 0-135
  TileType = raw / 4  →  0-33（34 种牌）
  copy     = raw % 4  →  0-3（每种 4 副）

TileType 编码：
  0-8:   万子 (1m-9m)
  9-17:  筒子 (1p-9p)
  18-26: 索子 (1s-9s)
  27-30: 风牌 (东南西北)
  31-33: 三元牌 (白发中)
```

## 牌山布局

```
索引:  0                    122  123  124  125  126  127  128  129  130  131  132  133  134  135
       |←— 摸牌区 (122张) —→|←— 王牌区 (14张) ——————————————————————————————→|

宝牌指示牌:   131(初始), 130, 129, 128, 127 (杠后追加)
里宝牌指示牌: 126(初始), 125, 124, 123, 122 (杠后追加)
岭上牌:       135, 134, 133, 132 (按杠顺序取用)
```

## 游戏流程

```
配牌 → [摸牌 → 行动 → 响应] × N → 局结束 → 下一局 / 半庄结束
         ↑                    ↑
      DrawPhase          ResponsePhase
         ↓                    ↓
      ActionPhase         RoundOver
```
