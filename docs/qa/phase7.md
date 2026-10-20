# QA log — Phase 7: "Open Markdown Files with smep" (default app from the menu)

Scope: the commit "Offer to become the default app for Markdown from the File menu"
(`src/defaults.rs`, `src/app.rs`, `src/keymap.rs`, `Cargo.toml`, README).
Rule: a round counts only if it finds nothing; any finding resets the count.

## What each system allows

| System | What the menu item does | Left to the user |
|---|---|---|
| macOS | `LSSetDefaultRoleHandlerForContentType("net.daringfireball.markdown", all roles, bundle id)` | nothing, when run from `smep.app`; a bare binary has no bundle id and gets a dialog saying so |
| Windows | registers the ProgId `smep.markdown` with its open command, `OpenWithProgids` on `.md .markdown .html .htm`, and the application under `Software\smep\Capabilities` + `RegisteredApplications` (so Settings lists smep by name); then opens `ms-settings:defaultapps?registeredAppUser=smep` | one click: choose smep next to `.md` in that page (Windows protects defaults) |
| Linux | writes `~/.local/share/applications/smep.desktop` with `Exec=<this binary> %f`, runs `update-desktop-database` (best effort) and `xdg-mime default smep.desktop text/markdown text/x-markdown` | nothing; `xdg-utils` must be installed |

The result shows in a dialog: "smep is the default for Markdown" or "One step left" with the instruction, or the error.

## Baseline

| Check | Windows (local) | Result |
|---|---|---|
| `cargo fmt --all --check` | exit 0 | clean |
| `cargo clippy --all-targets --locked -- -D warnings` | clean (default and `--no-default-features`) | |
| `cargo test --locked` × 3 | 97 passed, 0 failed each time (96 → 97: +1 unit, the registry round-trip) | |

## Round 1 — mechanism

- Windows: `registers_the_progid_the_extensions_and_the_application` registers under a scratch key `HKCU\Software\smep-test-<pid>` instead of the real root and reads back the open command (`"C:\tools\smep.exe" "%1"`), the `.md` OpenWithProgids entry, the `.md` file association and the `RegisteredApplications` pointer; registering twice is idempotent; `unregister` leaves nothing; the scratch key is deleted (checked after the run: 0 `smep-test-*` keys under `HKCU\Software`).
- Linux: `the_desktop_entry_names_the_running_binary` writes the entry into a temp dir and checks `Exec=/opt/smep/bin/smep %f` and the MIME line (runs on the CI Linux job).
- macOS: the LaunchServices call cannot run headless (no bundle in `cargo test`); the "no bundle" branch is the one a `cargo test` binary takes.

Round 1 result: 0 findings.

## Round 2 — static consistency

- README's "Make it the default" section matches the three branches of `defaults.rs` and names the menu item as the menus do ("Open Markdown Files with smep", File menu in the title bar and the native macOS menu).
- The registry layout matches `scripts/register-windows.ps1` (same ProgId, same four extensions, same command) plus the Capabilities registration the script lacks; the script's `-Remove` still clears what the app writes except `Software\smep` and the `RegisteredApplications` value (noted below).
- Dependencies: `winreg 0.55` and `core-foundation 0.10` were already in the lock file through gpui; no new transitive crates.

Round 2 result: 0 findings.

## Round 3 — platforms (CI)

Pending for the pushed commit.

## Not verified here

- The real registration and the Settings deep link on this machine were not run (it would register the debug build's path); the unit test covers the registry writes, the deep link is Windows' documented `ms-settings:defaultapps?registeredAppUser=<name>`.
- macOS end to end (needs `smep.app` on a Mac) and Linux `xdg-mime` end to end (CI runs the desktop-entry test only).
- `scripts/register-windows.ps1 -Remove` does not remove the new `Software\smep` and `RegisteredApplications\smep` entries; there is no "stop being the default" menu item yet.
