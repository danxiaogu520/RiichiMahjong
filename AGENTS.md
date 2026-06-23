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
- **已配置 git pre-commit hook 自动执行以下三项检查，任一失败则拒绝提交，因此请不要手动运行以下命令：**

```bash
cargo build --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
```

## Build & Run

```bash
cargo run -p riichi-client     # 终端 UI
cargo run -p riichi-server     # CLI 模式
cargo run -p riichi-test       # 交互式测试工具
```

## 测试方式

**不写单元测试。** 使用 `riichi-test` 交互式测试工具进行手动验证。

```bash
cargo run -p riichi-test
> a 12345m445p45678s          # analyze: 自动判断 3n+1/3n+2
> a 111222333m4455p           # 和了拆解
> dora 1m2m3m --indicator 1m # 宝牌计算
> points 4 30                 # 点数计算
```

手牌格式：`12345m445p45678s`（数字+花色字母，m=万 p=筒 s=索 z=字）

`analyze` 命令根据手牌张数自动分发：
- **3n+2**：和了 → 显示拆解；未和了 → 显示向听 + 打牌分析
- **3n+1**：听牌 → 显示听牌类型；未听牌 → 显示向听 + 进张/改良
- **3n**：拒绝分析

修改 riichi-logic 后，用 riichi-test 验证输出是否符合预期。

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
