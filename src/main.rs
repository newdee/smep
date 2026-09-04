//! smep — a Simple Markdown Editor & Previewer, written in Rust.
//!
//! `0.0.1` is a name-reservation release and contains no editor yet.
//! Development happens at <https://github.com/newdee/smep>.

fn main() {
    println!(
        "{} {} — a Simple Markdown Editor & Previewer, written in Rust",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );
    println!("This is a name-reservation release; the editor is not implemented yet.");
    println!("Follow progress at {}", env!("CARGO_PKG_REPOSITORY"));
}
