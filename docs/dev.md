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

`smep.exe` is linked as a GUI program (`windows_subsystem = "windows"` in
`main.rs`), so Explorer opens it without a terminal. Started from a
terminal, it attaches to that terminal's console first, so `--version`,
panics and anything on stderr still show in the window you ran `cargo run`
from, and `smep --version > v.txt` still lands in the file (the redirected
handles are kept). PowerShell is the exception: it does not wait for GUI
programs, so `$v = smep --version` stays empty there; `cmd /c` does it.

## macOS

Xcode command line tools, then `cargo run`.

## Releasing

1. Bump `version` in `Cargo.toml`, run `cargo check` so `Cargo.lock` follows,
   commit, push, wait for CI.
2. `cargo publish --locked` (the crates.io token comes from the gitignored
   `.env`: `set -a; . ./.env; set +a` first; in PowerShell,
   `Get-Content .env | ForEach-Object { if ($_ -match '^(\w+)=(.*)$') { Set-Item "Env:$($matches[1])" $matches[2] } }`).
3. `git tag -a vX.Y.Z -m "smep X.Y.Z" && git push origin vX.Y.Z`.

The tag runs `.github/workflows/release.yml`, which publishes a GitHub
release with a Linux tarball, a Windows zip and a universal macOS `smep.app`.
Run it by hand for an existing tag with
`gh workflow run release.yml -f tag=vX.Y.Z`.

The Homebrew cask and formula in [newdee/homebrew-tap](https://github.com/newdee/homebrew-tap)
(`Casks/smep.rb` installs `smep-vX.Y.Z-macos-universal.zip`, the `.app`,
and links its binary as the command; `Formula/smep.rb` installs
`smep-vX.Y.Z-macos-universal.tar.gz`, the bare universal binary the
release ships next to the zip; macOS only) follow the latest published
release on their own: a scheduled workflow there bumps both to the same
version, installs and tests each on a macOS runner, and commits the bump
only if that passes. To take a release at once instead of within six
hours: `gh workflow run smep.yml --repo newdee/homebrew-tap`. Both tests
run `smep --version`, which must print `smep X.Y.Z`. The cask exists
because a bare binary is a command-line tool to macOS: Finder and "Open
With" run it inside a Terminal window, and it cannot claim `.md` files.

The macOS app is signed and notarized only when these repository secrets
exist (they are the same six magpie uses). Without them it is ad-hoc signed
by the linker, which is enough for the formula (a binary run from the
terminal) and `cargo install`, while Finder refuses to open the bundle
until a Developer ID signature is there:

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

## Import and export

`src/convert.rs` turns PDF (`pdf-inspector`), Word (`docx-rust`), HTML
(`htmd`) and text into Markdown; `src/export.rs` turns Markdown into Typst
markup and compiles it (`typst-as-lib`, `typst-pdf`) with the fonts
installed on the machine (no fonts are embedded: Typst's own would add ~10 MB,
and every desktop has some; CJK needs a CJK font present). The export
test reads the text back out of the PDF it made. On Linux, CI installs
`fonts-dejavu-core fonts-noto-cjk` for that; a bare box without any font
cannot export.

## The look

One design, three outputs. `src/theme.rs` holds it as a `Palette` (six
colours, serif or not) plus the type every palette shares: the heading
scale (1.62 / 1.46 / 1.2 / 1.02 / 0.92 / 0.92 of the body), bold for the
first level and semibold below, a 2 px rule of ink under h1 and a 1 px
rule in the border colour under h2 (`heading_rule`), a 0.95 rem paragraph
gap, code at 0.9 of the body in a framed block with 8 px corners, a framed
table whose header row is ink on paper. From the palette:

- the window: `Palette::text_view_style` builds gpui-base's
  `TextViewStyle` (`with_heading`, `with_code_block`, `with_table*`). The
  renderer has no hook for the quote's bar, the rule's thickness, list
  markers or images, so those keep gpui-base's defaults on screen;
- an HTML page: `export::stylesheet` writes the same design as CSS, plus
  what only CSS can do here (the quote in the accent with a wash,
  alternating table rows, centred images, a print stylesheet);
- a PDF: `export::preamble` writes it as Typst `set` / `show` rules. The
  markup `to_typst` emits carries no colour, stroke or size of its own
  (a test checks), so the preamble is the whole look. A PDF takes the
  light theme of the preview's family (`PreviewTheme::light`): print is
  ink on paper.

`smep --css [THEME]` and `smep --typst [THEME]` print the two; saved as
`export.css` / `export.typ` in the config directory (`Settings::dir`,
next to `settings.toml`) they replace the built-in ones for every export,
whatever the theme. Tests never read the user's directory: a `Settings`
without a path has no directory.

## Icon

`assets/icon/logo.svg` is the design (a cinnabar seal with an M on the
Paper look's paper); `assets/icon/render.py` (Pillow) draws the same
geometry at 16 to 1024 px and packs `smep.ico`. Run it after changing the
design and commit the PNGs and the `.ico`. Where each platform takes it:

- Windows: `build.rs` embeds `smep.ico` as resource 1 with `winresource`
  (gpui loads icon 1 for the window; Explorer shows it). It needs the
  Windows SDK's `rc.exe`; without it the build warns and goes on without
  the icon.
- macOS: the release workflow makes `smep.icns` from the PNGs with
  `iconutil` and names it in `Info.plist` (`CFBundleIconFile`).
- Linux: `smep.desktop` names `Icon=smep`; "Open Markdown Files with smep"
  writes `smep-256.png` into `~/.local/share/icons/hicolor/256x256/apps/`,
  and the release tarball carries it as `smep.png`. The window sets
  `app_id` `smep` so the desktop matches it to the entry.

## Checks before pushing

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

CI runs the same three on Ubuntu, Windows and macOS, plus a release build.
