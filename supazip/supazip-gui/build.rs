//! Detect an optional JetBrains Mono TTF under `assets/fonts/`.
//!
//! The OFL font is not committed. When `JetBrainsMono-Regular.ttf` is
//! present, `theme.rs` embeds it via `include_bytes!`. Otherwise the GUI
//! keeps egui's built-in / system fallback.

use std::env;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let fonts_dir = manifest.join("../../assets/fonts");
    let regular = fonts_dir.join("JetBrainsMono-Regular.ttf");
    let bold = fonts_dir.join("JetBrainsMono-Bold.ttf");

    println!("cargo:rerun-if-changed={}", fonts_dir.display());
    println!("cargo:rerun-if-changed={}", regular.display());
    println!("cargo:rerun-if-changed={}", bold.display());
    println!("cargo:rustc-check-cfg=cfg(embed_jetbrains_mono)");
    println!("cargo:rustc-check-cfg=cfg(embed_jetbrains_mono_bold)");

    if regular.is_file() {
        println!("cargo:rustc-cfg=embed_jetbrains_mono");
    }
    if bold.is_file() {
        println!("cargo:rustc-cfg=embed_jetbrains_mono_bold");
    }
}
