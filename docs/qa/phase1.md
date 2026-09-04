# QA log — Phase 1: editor/preview skeleton

Scope: commit `c6c7e92` (skeleton on gpui-kit 0.6 / gpui-pre 0.3.3, CI, dev guide).
Rule: a round counts only if it finds nothing; any finding resets the count.
Acceptance needs three clean rounds in a row.

## Baseline (before the rounds)

| Check | Windows (local) | Result |
|---|---|---|
| `cargo fmt --all --check` | exit 0 | clean |
| `cargo clippy --all-targets --locked -- -D warnings` | exit 0 | clean (after removing one unused import) |
| `cargo test --locked` | 2 passed, 0 failed | `an_edit_in_the_editor_reaches_the_preview`, `the_initial_text_is_the_preview_source` |
| `cargo build` (debug, cold) | 1 m 11 s | 867 crates in the lock file |
| `cargo build --release --locked` | 1 m 32 s | `smep.exe` 21,876,736 bytes (20.9 MiB) |
| Manual smoke | `smep.exe README.md` | window titled `README.md — smep`; editor with line numbers and soft wrap; preview renders headings, bold, links, lists, code block; dark theme follows system |

Findings fixed before the rounds started:

- `source()` accessor was dead code in the non-test build (rustc warning). Removed; tests read the field.
- `AppContext as _` import unused in the test module (clippy `-D warnings`). Removed.

## Round 1 — static consistency (docs vs reality, dead configuration)

- `docs/dev.md` claimed Windows needs CMake. Checked `target/debug/build/aws-lc-sys-*/output`: it built with `Building with: CC`; `cmake`, `nasm`, `clang` are all absent from this machine's PATH. **Finding.** Doc corrected: cmake optional, C compiler required.
- `docs/dev.md` "Toolchain" paragraph contradicted itself ("no C toolchain requirements" then "a C compiler is needed"). **Finding.** Rewritten.
- `Cargo.toml` `include` still excludes `docs/` and `.github/`; `cargo package --list` to be re-checked before 0.1.0.
- CI `concurrency.cancel-in-progress: true` means a docs-only push cancels a running matrix. Acceptable for a solo repo, noted so pushes are batched.

Round 1 result: 2 findings → count resets.

## Round 2 — edge and degenerate inputs (clean streak 1)

Input: a 36-line file with a CJK heading plus emoji, CJK bold/italic/inline
code/link, a GFM table with a right-aligned column, a task list with a nested
item, a blockquote with strikethrough, a fenced Rust block with CJK inside,
an ordered list, a rule, a missing image, and a footnote. Run once with LF
(523 bytes) and once with CRLF (558 bytes) endings.

- Both variants render identically: every construct above appears in the preview; the editor shows 36 numbered lines in both cases, so `\r` does not create phantom lines.
- `normalize_input` in gpui-base leaves `\r` in place for multi-line inputs (checked the source). Saving will therefore preserve CRLF as-is; Phase 2 decides whether to normalise.
- Missing image renders as nothing (no alt text, no placeholder). Not a Phase 1 defect; carried forward.
- `cargo fmt`/`clippy`/`test` unchanged from baseline (code untouched in this round).

Round 2 result: 0 findings.

## Round 3 — error paths and repeatability (clean streak 2)

| Input | Output | Exit |
|---|---|---|
| nonexistent path | `smep: cannot read …\no-such-file.md: … (os error 2)` | 1 |
| 6-byte file with invalid UTF-8 | `smep: cannot read …\bad-utf8.md: stream did not contain valid UTF-8` | 1 |
| a directory | `smep: cannot read …\scratchpad: … (os error 5)` | 1 |
| `cargo test --locked` × 3 | `2 passed; 0 failed` each time | 0 |

- Every failure to read exits 1 with a message on stderr before the window opens. No panic, no empty window.
- Invalid UTF-8 is rejected rather than read lossily. Deliberate for now; revisit if real files hit it.

Round 3 result: 0 findings.

## Round 4 — platforms (CI on Ubuntu, Windows, macOS)

CI run 33829809919 for `c6c7e92`, all three jobs green (fmt, clippy `-D warnings`, test, release build):

| Job | Wall time | Release binary |
|---|---|---|
| ubuntu-latest | 11 m 21 s | `smep` 52,042,048 bytes (unstripped) |
| windows-latest | 22 m 41 s | `smep.exe` 21,871,616 bytes (+ 9.8 MB `.pdb`) |
| macos-latest | 15 m 54 s | `smep` 21,395,440 bytes |

- Both headless tests pass on all three platforms, so the edit→preview path is verified without a display.
- Annotation on every job: `actions/checkout@v4` targets Node 20, which the runners now force onto Node 24. **Finding.** Bumped to `actions/checkout@v5`.
- Linux binary is 2.4× the macOS one because release builds keep the symbol table by default there. **Finding** against the "single small static binary" goal. Added `[profile.release] strip = true`; the next CI run reports the new sizes.

Round 4 result: 2 findings → count resets.

## Round 5 — code correctness and repeatability after the fixes (clean streak 1)

Windows, on the tree with `strip = true` and `checkout@v5`:

| Check | Result |
|---|---|
| `cargo fmt --all --check` | exit 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | exit 0 |
| `cargo test --locked` × 3 | `2 passed; 0 failed` each time |
| `cargo build --release --locked` | 1 m 31 s, `smep.exe` 21,877,248 bytes (strip is a no-op on MSVC: symbols already live in the `.pdb`) |
| Release smoke `smep.exe README.md` | window titled `README.md — smep`, split renders |
| `cargo package --list` | 9 files: `.cargo_vcs_info.json`, `Cargo.lock`, `Cargo.toml`, `Cargo.toml.orig`, both licenses, `README.md`, `src/app.rs`, `src/main.rs`. No `docs/`, `.github/`, `.env`. |

Round 5 result: 0 findings.

## Round 6 — platforms again, on `2d840bc` (clean streak 2)

CI run 33831455782, all three jobs green:

| Job | Wall time (warm cache) | Release binary | Change vs round 4 |
|---|---|---|---|
| ubuntu-latest | 8 m 42 s | 37,664,304 bytes | −27.6 % |
| windows-latest | 14 m 56 s | 21,873,152 bytes (+ 9.8 MB `.pdb`) | ±0 (symbols were never in the exe) |
| macos-latest | 6 m 48 s | 16,034,304 bytes | −25.1 % |

- No annotations after the `checkout@v5` bump.
- Linux stays the largest: it links both the X11 and Wayland backends. LTO/opt-level tuning is a release-time decision, not a Phase 1 item.

Round 6 result: 0 findings.

## Round 7 — static consistency re-check (clean streak 3)

- Diff since the placeholder (`3e15dfc..HEAD`): 7 files, all within the Phase 1 scope (`ci.yml`, `Cargo.{toml,lock}`, `docs/dev.md`, `docs/qa/phase1.md`, `src/{app,main}.rs`).
- Risky calls in `src/`: exactly one `expect`, on `open_window` in `main.rs`. There is nothing to do without a window, so it stays.
- apt package list in `docs/dev.md` and `.github/workflows/ci.yml`: identical set for build packages; `dev.md` additionally lists runtime-only packages (`mesa-vulkan-drivers`, `vulkan-tools`, `fonts-noto-cjk`), which CI does not need.
- `rust-version = "1.97"` matches the toolchain Zed pins for this `gpui-pre` snapshot; CI builds on stable 1.98.
- Docs-only pushes now skip CI (`paths-ignore`), so QA-log commits no longer burn a 30-minute matrix.

Round 7 result: 0 findings.

## Verdict

Three consecutive clean rounds (5, 6, 7) after the last fix. Phase 1 is accepted for Windows, macOS and Linux-CI. **Not yet verified: a WSLg window on the user's machine** — the build packages are still missing there. That check reopens Phase 1 if it fails.

## Open items carried into Phase 2

- WSL build not yet verified: the 12 apt packages are not installed (needs sudo).
- `EditorState::default_value` normalises input; check what happens to CRLF files on save.
- Preview re-parses on every render while the source changes; debounce comes with Phase 2.
