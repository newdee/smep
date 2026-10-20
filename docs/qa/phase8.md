# QA log — Phase 8: no terminal window on Windows

Scope: the commit "Link smep.exe as a GUI program, so Windows opens it without a terminal"
(`src/main.rs`, `Cargo.toml`, `tests/command_line.rs`, README, `docs/dev.md`).
Rule: a round counts only if it finds nothing; any finding resets the count.

## The bug and its kind

Report: "为什么windows下打开，总是要打开一个终端". Every launch of `smep.exe` without a parent
console (Explorer, "Open with", a shortcut) came with a console window.

Cause: `smep.exe` was linked with the console subsystem, Rust's default; Windows gives such a
process a console when it has none. One place, structural (a missing declaration), no other
occurrence: `main.rs` is the only binary.

Fix: `#![cfg_attr(windows, windows_subsystem = "windows")]`. A GUI-subsystem process has no
console, so `main` first calls `AttachConsole(ATTACH_PARENT_PROCESS)` and then puts back any
standard handle the parent had redirected (AttachConsole replaces them with the console's).
`--version` and `--help` write with `writeln!` and ignore a closed pipe instead of panicking.

What was checked against new bugs: a terminal launch (plain, redirected to a file, piped,
PowerShell), an Explorer-style launch, the unreadable-file error, the three platforms on CI.

## Baseline

| Check | Windows (local) | Result |
|---|---|---|
| `cargo fmt --all --check` | exit 0 | clean |
| `cargo clippy --all-targets --locked -- -D warnings` (default and `--no-default-features`) | clean, clean | |
| `cargo test --locked` × 3 | 97 + 4 passed, 0 failed each time (+4: `tests/command_line.rs`) | |
| `cargo test --locked --no-default-features` | 90 + 4 passed | |
| `Cargo.lock` | one line added: `windows-sys 0.61.2` under smep (already in the tree) | |

## Round 1 — mechanism (Windows, real console and Explorer-style launch)

Explorer-style launch (`Start-Process`, no parent console), counting visible console windows
(`ConsoleWindowClass` + `CASCADIA_HOSTING_WINDOW_CLASS`) before and while smep runs:

| Build | Console windows before | During |
|---|---|---|
| old (console subsystem) | 1 | 2 |
| new (GUI subsystem) | 1 | 1 |

Inside a fresh real console (`cmd.exe` started with its own window, `probe.cmd`):

| Launch | Result |
|---|---|
| `smep --version` / `--help` / `smep no-such-file.md` on the console | all three printed on screen, in order (buffer dump) |
| `smep --version > f.txt` (cmd) | `smep 0.1.4` in the file |
| `smep --version \| findstr smep`, `--help \| findstr Usage` (cmd) | `smep 0.1.4`, `Usage: smep [FILE]` |
| `smep no-such-file.md 2> e.txt` (cmd) | `smep: cannot read …: 系统找不到指定的文件。 (os error 2)` in the file, exit 1 |
| `$x = & smep --version`, `smep --version > f.txt` (PowerShell) | empty: PowerShell does not wait for GUI programs (same for Notepad); documented, `cmd /c smep --version` captured `smep 0.1.4` from PowerShell |

The first version of the attach did not restore redirected handles: the cmd redirect and pipe
rows above were empty, the output went to the screen. Fixed before the rounds began.

`tests/command_line.rs`: `--version` and `--help` through a pipe, the error on stderr with exit 1,
and the PE optional header's Subsystem field = 2 (GUI) of the built `smep.exe`. Run both from the
sandbox (no console: AttachConsole fails) and from inside a real console (AttachConsole succeeds
and the pipe must survive): 4 passed, 0 failed in each.

Round 1 result: 0 findings.

## Round 2 — static consistency and code

- README and `docs/dev.md` say what the code does: GUI program, prints in a terminal, cmd redirect
  works, PowerShell capture does not (the `cmd /c` workaround is the one measured above).
- `Cargo.toml` `include` gained `tests/**`; `cargo package --list` shows `tests\command_line.rs`.
- `attach_parent_console` on other platforms is an empty function, so `main` reads the same
  everywhere; `windows-sys` is a `cfg(windows)` dependency only.
- Handle restore: only non-null, non-`INVALID_HANDLE_VALUE` handles are put back; a GUI process
  without redirection starts with null ones and keeps the console's.
- Closing the terminal still ends a smep started from it (the process is attached to that
  console); the old console build behaved the same, so nothing changed there.

Round 2 result: 0 findings.

## Round 3 — platforms (CI)

(filled in below from the CI run)
