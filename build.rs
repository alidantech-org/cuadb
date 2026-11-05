//! The server embeds the compiled web assets from `public/`. They are produced by Bun
//! (`bun run assets:build`) and are shipped inside the crates.io package. When they are
//! missing in a source checkout this tries to build them, and otherwise explains how.

use std::{path::Path, process::Command};

const FILES: &[&str] = &[
    "app.css",
    "app.js",
    "theme.js",
    "htmx.min.js",
    "sw.js",
    "site.webmanifest",
    "favicon.svg",
    "icon-192.png",
    "icon-512.png",
];

fn missing() -> Vec<&'static str> {
    FILES
        .iter()
        .copied()
        .filter(|f| !Path::new("public").join(f).exists())
        .collect()
}

fn main() {
    println!("cargo:rerun-if-changed=public");
    println!("cargo:rerun-if-changed=assets");
    println!("cargo:rerun-if-changed=build.rs");
    if missing().is_empty() {
        return;
    }
    if Path::new("package.json").exists() {
        let install = Command::new("bun")
            .args(["install", "--frozen-lockfile"])
            .status();
        if install.is_ok_and(|s| s.success()) {
            let _ = Command::new("bun").args(["run", "assets:build"]).status();
        }
    }
    let still = missing();
    if !still.is_empty() {
        panic!(
            "CuaDB web assets are missing from public/ ({}). Install Bun (https://bun.sh) and run `bun install && bun run assets:build`, then build again. Published crates already include them.",
            still.join(", ")
        );
    }
}
