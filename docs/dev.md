# Developing smep

smep is a single crate on top of [gpui-kit](https://github.com/longbridge/gpui-kit),
which pins Zed's GPUI (`gpui-pre`) from crates.io. There are no git dependencies,
so `cargo install smep` and `cargo build` work from a plain checkout.

## Toolchain

- Rust stable, at least the `rust-version` in `Cargo.toml`.
- A C compiler. smep itself is pure Rust and uses no tree-sitter, but the
  TLS stack GPUI ships (`aws-lc-sys`) builds C code. It uses `cmake` when
  present and falls back to the plain C compiler otherwise (verified on
  Windows with only the MSVC build tools installed).

## Linux (including WSL2 with WSLg)

Build dependencies on Debian/Ubuntu:

```sh
sudo apt install -y clang cmake lld \
  libasound2-dev libfontconfig-dev libwayland-dev libx11-xcb-dev \
  libxkbcommon-dev libxkbcommon-x11-dev libzstd-dev libvulkan1 \
  mesa-vulkan-drivers vulkan-tools fonts-noto-cjk
```

`fonts-noto-cjk` is only needed to render CJK text in the window; without it
those glyphs show as boxes.

Then:

```sh
cargo run -- README.md
```

### WSL notes

- Keep the build directory on the WSL filesystem. If the checkout lives under
  `/mnt/c`, set `CARGO_TARGET_DIR=$HOME/.cache/smep-target`.
- Check which Vulkan driver WSL picked up with `vulkaninfo --summary`.
  `dzn` (Direct3D 12 bridge) is hardware accelerated; `llvmpipe` is software
  rendering, which is slower but fine for development.
- If the window never appears under Wayland, force X11:
  `WAYLAND_DISPLAY='' cargo run`.

## Windows

Install the MSVC build tools (the Rust installer offers them), then
`cargo run`. Nothing else is required. Windows is verified by CI on every
push; day-to-day development happens on Linux.

## macOS

Xcode command line tools, then `cargo run`.

## Releasing

1. Bump `version` in `Cargo.toml`, run `cargo check` so `Cargo.lock` follows,
   commit, push, wait for CI.
2. `cargo publish --locked` (the crates.io token comes from the gitignored
   `.env`: `set -a; . ./.env; set +a` first; in PowerShell,
   `Get-Content .env | ForEach-Object { if ($_ -match '^(\w+)=(.*)$') { Set-Item "Env:$($matches[1])" $matches[2] } }`).
3. `git tag -a vX.Y.Z -m "smep X.Y.Z" && git push origin vX.Y.Z`.

The tag runs `.github/workflows/release.yml`, which drafts a GitHub release
with a Linux tarball, a Windows zip and a universal macOS `smep.app`.
Run it by hand for an existing tag with
`gh workflow run release.yml -f tag=vX.Y.Z`.

The macOS app is signed and notarized only when these repository secrets
exist (they are the same six magpie uses):

| Secret | Contents |
|---|---|
| `APPLE_CERTIFICATE` | base64 of the Developer ID Application `.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | the `.p12` password |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Name (TEAMID)` |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | notarization: Apple ID, an app-specific password, the team id |

GitHub cannot copy secrets between repositories, so they have to be entered
once more. `scripts/set-apple-secrets.sh path/to/DeveloperID.p12` (or the
`.ps1` twin on Windows) asks for each value with hidden input and stores all
six; nothing is echoed or written to disk.

## Checks before pushing

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

CI runs the same three on Ubuntu, Windows and macOS, plus a release build.
