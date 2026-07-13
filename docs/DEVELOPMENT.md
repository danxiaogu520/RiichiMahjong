# 开发指南

## 环境

安装 Rust stable 和 Cargo。项目是 Cargo workspace，默认成员是 `riichi-debug`，所有 crate 可用 `--workspace` 一起处理。构建目录 `target/`、Python 虚拟环境和工具缓存不进入仓库。

## 常用命令

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo run -p riichi-debug
cargo run -p riichi-server
cargo run -p riichi-test
```

提交前至少运行格式检查、workspace 测试和与改动模块对应的测试。规则或结算改动应补充可复现的回归测试，优先放在对应 crate；跨模块流程放在 `crates/riichi-engine/tests/`。

## 改动原则

1. 先确认规则口径，再修改 engine 或 logic。
2. 保持依赖方向：UI 不直接实现规则，AI 不修改引擎状态。
3. 不把随机数、网络或终端 I/O 引入纯算法模块。
4. 对外消息的新增字段要同步更新视图、转换和测试。
5. 变更行为时同时更新 `docs/` 中受影响的说明。

## 排错入口

- 牌编码或牌山问题：`riichi-core/src/tile.rs`、`wall.rs`。
- 和牌、役种、点数问题：`riichi-logic/src/`。
- 合法行动和时序问题：`riichi-engine/src/legal.rs`、`action.rs`、`call.rs`、`round.rs`。
- 流局和终局问题：`ryukyoku.rs`、`settlement.rs`。
- UI 不一致：先检查 `riichi-proto` 的 view 转换，再检查客户端渲染。

## 提交前检查

确认 `git status` 中没有 `target/`、`.venv/`、`.mimocode/`、`.agents/` 或 `.codex/`；确认生成文件没有被 `git add`，再提交源码、测试和文档。
