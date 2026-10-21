//! The docs must not claim what the code does not do.
//!
//! Agent context files grow by addition and rarely by deletion, so a claim that was once true
//! stays in the file after the behaviour changed. CLAUDE.md said a screenshot "fails loudly when
//! `DebugCaptureTarget` is absent rather than falling back to window capture"; the code falls back
//! to window capture, and only fails loudly when `offscreen` is asked for by name.
//!
//! These are the checks that catch that class of error. Each one names a claim and the code that
//! makes it true or false.

use std::fs;

/// The repository root, from this file's location.
fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn read(relative: &str) -> String {
    fs::read_to_string(root().join(relative))
        .unwrap_or_else(|e| panic!("reading {relative} failed: {e}"))
}

/// CLAUDE.md must not say the screenshot falls back to nothing, because it does: it falls back to
/// window capture, and a window capture can be stale.
#[test]
fn claude_md_does_not_claim_the_screenshot_refuses_to_fall_back() {
    let claude = read("CLAUDE.md");
    let lower = claude.to_lowercase();

    assert!(
        !lower.contains("fails loudly when `debugcapturetarget` is absent"),
        "CLAUDE.md still claims a screenshot fails loudly without DebugCaptureTarget. It does not: it \
         falls back to window capture. The claim is in CLAUDE.md."
    );
    assert!(
        lower.contains("window capture") || lower.contains("`window`"),
        "CLAUDE.md should say what the capture falls back to"
    );
}

/// The claim that the plugin pins is the one the code implements: capture reads the game's own
/// pixels, and input goes into the message stream.
#[test]
fn claude_md_keeps_the_two_rules_the_plugin_pins() {
    let claude = read("CLAUDE.md").to_lowercase();
    assert!(
        claude.contains("offscreen"),
        "CLAUDE.md should name the offscreen capture path"
    );
    assert!(
        claude.contains("messages<keyboardinput>") || claude.contains("keyboardinput"),
        "CLAUDE.md should name the input message stream the plugin writes into"
    );
}

/// The library docs must not tell a reader to pin a Bevy version the crate does not use. The
/// check that covers this for every doc is `the_docs_name_the_bevy_version_the_crate_uses`.
#[test]
fn the_crate_docs_name_the_bevy_version_the_crate_uses() {
    let manifest = read("Cargo.toml");
    let bevy_version = manifest
        .lines()
        .find(|line| line.trim_start().starts_with("bevy = "))
        .and_then(|line| line.split('"').nth(1))
        .map(|version| version.split('.').take(2).collect::<Vec<_>>().join("."))
        .expect("Cargo.toml names a bevy version");

    let lib_docs = read("src/lib.rs");
    for line in lib_docs.lines() {
        let trimmed = line.trim();
        if !trimmed.contains("bevy = { version") {
            continue;
        }
        let named = trimmed
            .split('"')
            .nth(1)
            .unwrap_or_default()
            .split('.')
            .take(2)
            .collect::<Vec<_>>()
            .join(".");
        assert_eq!(
            named, bevy_version,
            "src/lib.rs names Bevy {named}, but the crate uses {bevy_version}. Line: {trimmed}"
        );
    }
}

/// The README must not tell a reader to register a BRP method the plugin already registers. Doing
/// so adds `RemotePlugin` a second time, which Bevy rejects by name, so the game panics on start.
#[test]
fn the_readme_does_not_tell_the_reader_to_register_a_method_the_plugin_registers() {
    let readme = read("README.md");
    let plugin = read("crates/bevy_debugger_bevy/src/lib.rs");

    // The plugin registers these itself.
    for method in [
        "bevy_debugger/screenshot",
        "bevy_debugger/capture",
        "bevy_debugger/input",
        "bevy_debugger/commands",
        "bevy_debugger/scene",
        "bevy_debugger/quit",
        "bevy_debugger/status",
    ] {
        assert!(
            plugin.contains(&format!("\"{method}\"")),
            "the plugin is expected to register {method}"
        );
    }

    assert_no_hand_registration("README.md", &readme);
}

/// The same goes for the book: a guide that tells the reader to register a method the plugin owns
/// makes the game panic on start.
#[test]
fn the_book_does_not_tell_the_reader_to_register_a_method_the_plugin_registers() {
    assert_no_hand_registration("book/SCREENSHOT_SETUP.md", &read("book/SCREENSHOT_SETUP.md"));
}

/// A registration is written as `.with_method("name", handler)`. No doc may tell the reader to do
/// that for a method the plugin owns.
fn assert_no_hand_registration(what: &str, text: &str) {
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(".with_method(\"") {
            let name = rest.split('"').next().unwrap_or_default();
            if name.starts_with("bevy_debugger/") {
                panic!(
                    "{what} tells the reader to register {name} by hand, but the plugin already \
                     registers it. Adding it again adds `RemotePlugin` twice, which Bevy rejects, so \
                     the game panics on start. Line: {trimmed}"
                );
            }
        }
    }
}

/// The docs must name the Bevy version the crate uses. A reader who follows a doc that names an
/// old version gets a different API than the one described.
#[test]
fn the_docs_name_the_bevy_version_the_crate_uses() {
    let manifest = read("Cargo.toml");
    let bevy_version = manifest
        .lines()
        .find(|line| line.trim_start().starts_with("bevy = "))
        .and_then(|line| line.split('"').nth(1))
        .map(|version| version.split('.').take(2).collect::<Vec<_>>().join("."))
        .expect("Cargo.toml names a bevy version");

    for (what, text) in [
        ("README.md", read("README.md")),
        ("book/SCREENSHOT_SETUP.md", read("book/SCREENSHOT_SETUP.md")),
        ("src/lib.rs", read("src/lib.rs")),
    ] {
        for line in text.lines() {
            let trimmed = line.trim();
            if !trimmed.contains("bevy = { version") && !trimmed.contains("bevy = { version = \"") {
                continue;
            }
            let named = trimmed
                .split('"')
                .nth(1)
                .unwrap_or_default()
                .split('.')
                .take(2)
                .collect::<Vec<_>>()
                .join(".");
            assert_eq!(
                named, bevy_version,
                "{what} names Bevy {named}, but the crate uses {bevy_version}. Line: {trimmed}"
            );
        }
    }
}
