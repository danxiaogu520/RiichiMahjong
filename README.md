# RiichiMahjong

日本麻将（立直麻将）引擎，Rust 实现。

## 项目结构

```
crates/
  riichi-core/      纯数据结构：牌、手牌、副露、牌山、玩家状态
  riichi-logic/     纯算法：向听数、判役、算点、宝牌、有效牌分析
  riichi-ai/        决策层：打牌选择、副露决策、立直决策
  riichi-engine/    状态机：游戏流程、回合管理、副露处理
  riichi-server/    服务端：房间管理、AI 控制、CLI 显示
  riichi-client/    客户端：ratatui 终端 UI
  riichi-proto/     通信协议：客户端-服务端消息定义
```

## 依赖关系

```
riichi-proto
     ↑
riichi-core
     ↑
riichi-logic
     ↑                  ↑
riichi-ai            riichi-engine
     ↑                  ↑
     └── riichi-server ← riichi-proto
             ↑
        riichi-client ← riichi-proto
```

## 快速开始

```bash
# 构建
cargo build

# 运行终端 UI（默认）
cargo run -p riichi-client

# 运行 CLI 模式
cargo run -p riichi-server
```

## 规则实现

- 半庄（东一局～南四局）
- 立直、一发、双立直
- 门前清自摸和、岭上开花、抢杠和
- 食替限制
- 振听判定（舍牌振听、同巡振听、立直振听）
- 宝牌、里宝牌、赤宝牌
- 七对子、国士无双
- 四风连打、四家立直、四杠散了、九种九牌（途中流局）

## 技术细节

- **牌编码**：`Tile(u8)` 0-135，每种牌 4 副。`TileType(u8)` 0-33（34 种）
- **牌山**：`Wall` struct 管理 136 张牌，索引 0-121 为摸牌区，122-135 为王牌区
- **向听计算**：查表法（预计算的 base-5 编码查找表）
- **役种判定**：枚举所有可能的手牌分解，高点法取最优

## License

MIT
