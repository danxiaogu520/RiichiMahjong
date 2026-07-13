# 架构

## 设计目标

项目把“牌和手牌数据”“纯算法”“对局状态”“外部交互”分层。规则计算尽量保持无副作用，状态变化集中在 `riichi-engine`，这样算法可以被客户端、AI 和测试工具复用。

## Crate 关系

```text
riichi-core
├── riichi-logic
│   └── riichi-ai
├── riichi-engine
│   └── riichi-server
│       └── riichi-client
└── riichi-proto ───────────────┘
```

- `riichi-core`：基础类型和牌山，不负责判定一局游戏是否结束。
- `riichi-logic`：向听、分解、和牌、役种、符数、点数、宝牌和牌效分析。
- `riichi-ai`：把逻辑层结果转成打牌、鸣牌或立直选择。
- `riichi-engine`：维护 `GameState`，执行合法行动、响应窗口、流局、和了与结算。
- `riichi-proto`：定义隐藏他人手牌后的 `GameStateView`，以及客户端行动和服务端事件。
- `riichi-server`：把引擎包装为异步游戏回路，并通过 channel 接收玩家/AI 行动。
- `riichi-client`：把状态视图和分析结果渲染为终端 UI。
- `riichi-test`：用于手牌解析和逻辑验证的轻量 CLI。

规则变化应优先落在 engine 的规则/结算模块和 logic 的判定模块；UI 不应自行推导点数或改变合法性。

## 核心数据

`Tile(u8)` 使用 0–135 表示一张具体牌；`TileType` 使用 0–33 表示牌种，每种牌有四张具体牌。万、筒、索分别占 0–8、9–17、18–26，风牌占 27–30，三元牌占 31–33。

`Wall` 管理 136 张牌。前 122 张是摸牌区，后 14 张是王牌；宝牌指示牌、里宝牌指示牌和岭上牌的位置由牌山接口统一管理。

## 一局数据流

```text
初始化 → 配牌 → 摸牌 → 当前玩家行动
                  ↓             ↓
              引擎状态 ← 响应窗口 ← 其他玩家
                  ↓
       和了 / 流局 / 继续下一回合
                  ↓
                结算
```

外部输入先经过 `riichi-engine::legal` 判定，再由 action/round/call 模块修改状态。结束原因统一进入 settlement；多家荣和或多家流局满贯必须先完成全部点数变化，再执行一次终局判定。

## 测试边界

- core 测试牌山、牌型基础不变量。
- logic 测试向听、役种、符数、点数与牌效。
- engine 测试行动时序、流局和结算。
- server/proto 测试消息转换、序列化和回路边界。
- `tests/half_game.rs` 提供跨模块的半庄级回归场景。
