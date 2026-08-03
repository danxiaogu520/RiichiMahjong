# RiichiMahjong

一个以 Rust 编写的四人立直麻将引擎，包含规则状态机、和牌/计分算法、AI 决策、服务端回路和 ratatui 终端客户端。

项目已支持浏览器/Tauri 预览下的熟人房间联机：房主可以配置 0–3 个固定基础 AI，至少一名真人即可开局；真人断线 30 秒后由 AI 临时接管，重连后恢复真人控制。房间和连接状态目前保存在服务端内存中。规则口径和已知实现缺口见 [`docs/RULES.md`](docs/RULES.md)。

## 快速开始

需要 Rust stable 和 Cargo。首次构建会生成本地 `target/`，该目录已被忽略。

```bash
cargo build                 # 构建默认客户端及其依赖
cargo test --workspace      # 运行整个 workspace 的测试
cargo run -p riichi-debug   # 启动终端调试客户端
cd tauri-app && npm run build # 构建浏览器/Tauri 前端预览
```

客户端默认启动一桌由 AI 控制的本地对局；终端交互需要可用的 TTY。纯算法和规则状态机也可以单独作为 crate 使用。

## Workspace

| Crate | 职责 |
| --- | --- |
| `riichi-core` | 牌、手牌、副露、牌山和玩家基础数据 |
| `riichi-logic` | 向听、牌型分解、役种、符数、点数和牌效分析 |
| `riichi-ai` | 固定基础 AI 的打牌、鸣牌和立直决策 |
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
- [`docs/DEPLOYMENT.md`](docs/DEPLOYMENT.md)：Tauri 构建、Linux 服务和 HTTPS/WSS 部署

## 许可

MIT，详见 [`LICENSE`](LICENSE)。
