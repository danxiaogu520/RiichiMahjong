# AGENTS.md

## 协作规范

**每次修改必须 git commit + git push。** 不要留本地未推送的提交。

```bash
git add <changed-files>
git commit -m "<concise description>"
git push
```

- commit message 简洁描述改了什么
- 不要提交 `target/` 目录（已在 .gitignore 中）
- 每个逻辑改动一个 commit
- **提交前必须通过以下三项检查，全部通过才能 commit + push：**

```bash
cargo build --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
```

> 已配置 git pre-commit hook 自动执行以上三项检查。任一失败则拒绝提交。

## Build & Run

```bash
cargo build                    # 构建所有 crate
cargo run -p riichi-client     # 终端 UI
cargo run -p riichi-server     # CLI 模式
cargo fmt                      # 格式化
cargo clippy --workspace       # lint
```

## Workspace 结构

详见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)

```
crates/
  riichi-core/      纯数据结构
  riichi-logic/     纯算法（向听/判役/算点/有效牌）
  riichi-ai/        决策层
  riichi-engine/    状态机
  riichi-server/    服务端
  riichi-client/    客户端（ratatui TUI）
  riichi-proto/     通信协议
```

## 关键约定

- 新代码不加注释（除非明确要求）
- 字牌显示：`1z`=东, `2z`=南, `3z`=西, `4z`=北, `5z`=白, `6z`=发, `7z`=中
- 庄家 = `PlayerId((round - 1) % 4)`，Round 1-4 = 东场，5-8 = 南场
