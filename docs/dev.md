# Developing smep

smep is a single crate on top of [gpui-kit](https://github.com/longbridge/gpui-kit),
which pins Zed's GPUI (`gpui-pre`) from crates.io. There are no git dependencies,
so `cargo install smep` and `cargo build` work from a plain checkout.

## Toolchain

- Rust stable, at least the `rust-version` in `Cargo.toml`.
- No tree-sitter, no C toolchain requirements beyond what the platform SDK
  already provides. `cmake` and a C compiler are still needed because the
  TLS stack GPUI ships (`aws-lc-sys`) builds C code.

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

Install the MSVC build tools (the Rust installer offers them) and CMake, then
`cargo run`. Windows is verified by CI on every push; day-to-day development
happens on Linux.

## macOS

Xcode command line tools, then `cargo run`.

## Checks before pushing

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

CI runs the same three on Ubuntu, Windows and macOS, plus a release build.
