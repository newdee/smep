//! The built `smep` binary from the outside: what it prints for `--version`
//! and `--help`, and, on Windows, that it is linked as a GUI program (no
//! console window when started from Explorer).

use std::process::Command;

const SMEP: &str = env!("CARGO_BIN_EXE_smep");

#[test]
fn version_prints_the_crate_version() {
    let output = Command::new(SMEP).arg("--version").output().unwrap();
    assert!(output.status.success(), "{:?}", output.status);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        format!("smep {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn help_prints_the_usage() {
    let output = Command::new(SMEP).arg("--help").output().unwrap();
    assert!(output.status.success(), "{:?}", output.status);
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.starts_with("Usage: smep [FILE]"), "{text}");
    assert!(text.contains("--version"), "{text}");
}

#[test]
fn an_unreadable_file_is_reported_on_stderr() {
    let missing = std::env::temp_dir().join(format!("smep-missing-{}.md", std::process::id()));
    let output = Command::new(SMEP).arg(&missing).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8_lossy(&output.stderr);
    assert!(text.starts_with("smep: cannot read "), "{text}");
}

/// The PE optional header's Subsystem field: 2 is IMAGE_SUBSYSTEM_WINDOWS_GUI,
/// 3 is IMAGE_SUBSYSTEM_WINDOWS_CUI (the Rust default, which opens a terminal).
#[cfg(windows)]
#[test]
fn the_binary_is_a_gui_program() {
    let image = std::fs::read(SMEP).unwrap();
    let u16_at = |at: usize| u16::from_le_bytes([image[at], image[at + 1]]);
    let u32_at = |at: usize| u32::from_le_bytes(image[at..at + 4].try_into().unwrap());
    assert_eq!(&image[..2], b"MZ");
    let pe = u32_at(0x3c) as usize;
    assert_eq!(&image[pe..pe + 4], b"PE\0\0");
    // COFF header (20 bytes) follows the signature; the optional header
    // follows it, with Subsystem at offset 68 in both PE32 and PE32+.
    let subsystem = u16_at(pe + 4 + 20 + 68);
    assert_eq!(subsystem, 2, "IMAGE_SUBSYSTEM_WINDOWS_GUI");
}
