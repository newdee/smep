# smep — a Simple Markdown Editor & Previewer, written in Rust

[![crates.io](https://img.shields.io/crates/v/smep.svg)](https://crates.io/crates/smep)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

**smep** is a markdown editor with live preview, built in pure Rust on the
[GPUI](https://www.gpui.rs) framework (the UI toolkit behind the Zed editor).

## Status

`0.0.1` is a name-reservation release. It ships a single binary that prints
its version and exits. No editing or preview functionality exists yet.

## Goals

- Pure Rust, no web view, no Electron.
- GPU-accelerated UI via GPUI.
- Side-by-side editor and rendered preview, updated as you type.
- CommonMark + GFM (tables, task lists, strikethrough, footnotes).
- Fast startup, low memory, single static binary.

## Install

```sh
cargo install smep
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this work by you shall be dual licensed as above, without any
additional terms or conditions.
