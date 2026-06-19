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
cargo test -p mahjong-core     # test a single crate
cargo test -p mahjong-yaku
cargo test -p mahjong-engine
cargo test -- --ignored        # run ignored tests (marked with #[ignore])
```

No CI, no linter config, no formatter config. Use `cargo fmt` and `cargo clippy` defaults.

## Workspace Structure

```
crates/
  mahjong-core/     # tile, hand, meld, wall, player data types
  mahjong-yaku/     # win detection, yaku detection, fu/scoring calculation
  mahjong-engine/   # game state machine, turn flow, call handling
  mahjong-server/   # CLI interactive game (human vs 3 AI)
```

**Dependency graph**: `core` ← `yaku` ← `engine` ← `server`

`mahjong-yaku` does NOT depend on `rand`. Only `core`, `engine`, and `server` use `rand 0.8`.

## Architecture

This is a Japanese Riichi Mahjong (日麻) engine. All comments are in Chinese.

**Tile encoding**: `Tile(u8)` 0-135, 4 copies per tile type. `TileType(u8)` 0-33 (34 types). Conversion: `tile.raw() / 4 == tile_type.0`.

**Game flow** (`engine/src/game.rs`):
- `GameState` holds all state: players, wall, dora, phase, events
- Phases: `DrawPhase` → `ActionPhase` → `ResponsePhase` → `RoundOver`
- `drawn_tile` is a buffer — the drawn tile stays outside hand until the player acts
- `start_round()` deals 13+1, sets up wall/dora; `draw()` advances `current_index`

**Call system** (`engine/src/call.rs`): `detect_calls()` finds all valid chi/pon/minkan/ron for a discarded tile. Priority: ron > minkan > pon > chi.

**Yaku detection** (`yaku/src/win_check.rs`):
- `check_win()` is the entry point: furiten check → win shape → yaku detection → dora → fu → scoring
- `detect_yaku()` evaluates all decompositions, picks the highest-scoring yaku combination
- `decompose_hand()` returns all standard + seven pairs + kokushi decompositions

## Key Conventions

- **No comments in new code** unless explicitly requested
- Winds: East=0, South=1, West=2, North=3 (`PlayerId` maps to wind via `wind_from_index`)
- 字牌 display: `1z`=East, `2z`=South, `3z`=West, `4z`=North, `5z`=Haku, `6z`=Hatsu, `7z`=Chun
- Wall layout: indices 0-121 = draw area, 122-135 = dead wall (dora indicators + rinshan tiles)
- `RINSHAN = 135`, `DORA_INDICATOR = 131`, `HAITEI = 121` — hardcoded in `game.rs`
- Dealer = `PlayerId((round - 1) % 4)`. Round 1-4 = East, 5-8 = South. `round > 8` = game over.
- Honba (本场) increments on dealer continuation, resets on rotation

## Known Gaps

- `fu.rs`: wait type fu (单骑/边张/坎张 +2) is not implemented (marked TODO)
- `server/src/main.rs`: AI players use random discard — no strategy
- Several `#[ignore]` tests in `engine/src/game.rs` depend on `start_round` (marked TODO)
