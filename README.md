# RiichiMahjong

一个以 Rust 编写的四人立直麻将引擎，包含规则状态机、和牌/计分算法、AI 决策、服务端回路和 ratatui 终端客户端。

项目目前面向本地实验与规则验证。规则口径和已知实现缺口见 [`docs/RULES.md`](docs/RULES.md)，不要把尚未完成的行为当作线上稳定协议。

## 快速开始

需要 Rust stable 和 Cargo。首次构建会生成本地 `target/`，该目录已被忽略。

```bash
cargo build                 # 构建默认客户端及其依赖
cargo test --workspace      # 运行整个 workspace 的测试
cargo run -p riichi-debug   # 启动终端调试客户端
```

客户端默认启动一桌由 AI 控制的本地对局；终端交互需要可用的 TTY。纯算法和规则状态机也可以单独作为 crate 使用。

## Workspace

| Crate | 职责 |
| --- | --- |
| `riichi-core` | 牌、手牌、副露、牌山和玩家基础数据 |
| `riichi-logic` | 向听、牌型分解、役种、符数、点数和牌效分析 |
| `riichi-ai` | 打牌、鸣牌和立直决策 |
| `riichi-engine` | 局面状态、行动合法性、回合流程和结算 |
| `riichi-proto` | 客户端与服务端之间的序列化消息 |
| `riichi-session` | 游戏会话、玩家命令和事件通道 |
| `riichi-server` | 网络、房间和连接服务 |
| `riichi-debug` | ratatui 终端调试界面 |

依赖方向、主要数据流和牌编码见 [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)。

## 文档

- [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md)：开发、测试、格式化和排错
- [`docs/RULES.md`](docs/RULES.md)：固定规则口径、结算顺序和已知缺口
- [`docs/PROTOCOL.md`](docs/PROTOCOL.md)：消息边界和状态视图
- [`docs/ROADMAP.md`](docs/ROADMAP.md)：当前限制与后续工作

## 许可

MIT，详见 [`LICENSE`](LICENSE)。
