# AGENTS.md

## 协作规范 (Collaboration Rules)

**每次修改必须 git commit + git push。** 不要留本地未推送的提交。

```bash
# 标准工作流
git add <changed-files>
git commit -m "<concise description>"
git push
```

- commit message 用中文或英文均可，但要简洁描述改了什么
- 不要提交 `target/` 目录（已在 .gitignore 中）
- 每个逻辑改动一个 commit，不要把无关改动混在一起
- 推送前确认 `cargo build` 编译通过

## Build & Test

```bash
cargo build                    # build all crates
cargo run -p riichi-client     # launch terminal UI
cargo run -p riichi-server     # launch CLI game
```

No CI, no linter config, no formatter config. Use `cargo fmt` and `cargo clippy` defaults.

## Workspace Structure

```
crates/
  riichi-core/      # 纯数据结构：tile, hand, meld, wall, player, game_types, player_state
  riichi-logic/     # 纯算法：shanten, yaku, fu, scoring, dora, analysis, win_check, acceptance
  riichi-ai/        # 决策层：打牌选择, 副露决策, 立直决策
  riichi-engine/    # 纯状态机：游戏状态, 回合流程, 副露处理
  riichi-server/    # 游戏服务：房间管理, AI 控制, CLI 显示
  riichi-client/    # 客户端：ratatui 终端 UI
  riichi-proto/     # 通信协议：客户端-服务端消息类型
```

**Dependency graph**:
```
riichi-proto  (standalone: serde)
     ↑
riichi-core   (tile, hand, meld, wall, player, game_types, player_state)
     ↑
riichi-logic  (shanten, yaku, fu, scoring, dora, analysis, win_check, acceptance)
     ↑                  ↑
riichi-ai            riichi-engine  (纯状态机, 依赖 core + logic + ai)
     ↑                  ↑
     └── riichi-server ← riichi-proto
             ↑
        riichi-client ← riichi-proto
```

- `riichi-logic` does NOT depend on `rand`. Only `core`, `engine`, and `server` use `rand 0.8`.
- `riichi-proto` defines `GameStateView`, `PlayerView`, `AnalysisInfo` for client-server communication.

## Architecture

This is a Japanese Riichi Mahjong (日麻) engine. All comments are in Chinese.

**Tile encoding**: `Tile(u8)` 0-135, 4 copies per tile type. `TileType(u8)` 0-33 (34 types). Conversion: `tile.raw() / 4 == tile_type.0`.

**Game flow** (`engine/src/game.rs`):
- `GameState` holds all state: players, wall (as `Wall` struct), dora, phase, events
- Phases: `DrawPhase` → `ActionPhase` → `ResponsePhase` → `RoundOver`
- `drawn_tile` is a buffer — the drawn tile stays outside hand until the player acts
- `start_round()` uses `Wall::new(rng)` to deal; `draw()` calls `wall.draw()`

**Call system** (`engine/src/call.rs`): `detect_calls()` finds all valid chi/pon/minkan/ron for a discarded tile. Priority: ron > minkan > pon > chi.

**Yaku detection** (`riichi-logic/src/win_check.rs`):
- `check_win()` is the entry point: furiten check → win shape → yaku detection → dora → fu → scoring
- `detect_yaku()` evaluates all decompositions, picks the highest-scoring yaku combination
- `decompose_hand()` returns all standard + seven pairs + kokushi decompositions

## Key Conventions

- **No comments in new code** unless explicitly requested
- Winds: East=0, South=1, West=2, North=3 (`PlayerId` maps to wind via `wind_from_index`)
- 字牌 display: `1z`=East, `2z`=South, `3z`=West, `4z`=North, `5z`=Haku, `6z`=Hatsu, `7z`=Chun
- Wall layout: indices 0-121 = draw area, 122-135 = dead wall (dora indicators + rinshan tiles)
- Dora indicators: 131(initial), 130, 129, 128, 127 (after kan). Ura-dora: 126, 125, 124, 123, 122
- Rinshan tiles: 135, 134, 133, 132 (in kan order)
- Dealer = `PlayerId((round - 1) % 4)`. Round 1-4 = East, 5-8 = South. `round > 8` = game over.
- Honba (本场) increments on dealer continuation, resets on rotation

## Known Gaps

- `riichi-logic/src/fu.rs`: wait type fu (单骑/边张/坎张 +2) is not implemented (marked TODO)
- `riichi-ai`: AI players use shanten-based strategy (basic, no call decisions). `call_decision.rs` always passes.
- Engine still has some computation methods (is_tenpai, check_tsumo, etc.) — planned for future extraction
