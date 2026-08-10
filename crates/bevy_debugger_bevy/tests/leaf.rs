//! **The crate boundary, enforced — and here the boundary is the OS.**
//!
//! Every other dependency ratchet in this workspace asks "is this crate still engine-free?". This one
//! cannot: the plugin's whole job is to live inside a Bevy `App`. What it must never do is reach the
//! machine the game is running on.
//!
//! That property is the reason this crate exists at all. Capturing a window requires the window to be
//! raised — stealing focus, possibly switching workspaces — and driving the OS keyboard sends
//! keystrokes to whatever application actually has focus, which may be somebody's editor. Measured on
//! the game this was built for: **7,188 distinct colours** captured with the window focused, **1** with
//! another app in front.
//!
//! So the guarantee is structural rather than careful. `bevy_debugger/screenshot` reads an `Image` a
//! camera rendered to, and `bevy_debugger/input` writes into the same in-process `Messages` buffer
//! `bevy_winit` appends to. Neither can leak outside the process, *because there is nothing linked
//! here that could do it* — and the input half got **stronger** when it moved from `ButtonInput` to
//! the message stream, not weaker: a `Messages<KeyboardInput>` buffer is an ECS resource, and reaching
//! a real event loop would take a dependency this file forbids.
//!
//! These two tests are that claim, made checkable. Widening either list is a design decision, so it
//! should cost a deliberate edit in this file rather than passing silently in a build.

use std::path::{Path, PathBuf};

/// Everything this crate is allowed to depend on.
///
/// `bevy`/`bevy_remote` are the plugin and the protocol; `serde`/`serde_json` parse method params;
/// `image` encodes the captured PNG. None of the five can synthesise an operating-system event, and
/// that is the entire argument for why injected input cannot escape the game process.
const ALLOWED_DEPS: &[&str] = &["bevy", "bevy_remote", "serde", "serde_json", "image"];

/// Names whose presence would mean the OS boundary has been crossed.
///
/// The input-synthesis crates and platform APIs are the obvious half. `winit` is here because it owns
/// the real event loop — reading input from it rather than from `ButtonInput` would reintroduce the
/// dependency on which window has focus. `unsafe` is here because every listed API is reached through
/// it, so it is the cheapest possible tripwire for an FFI route nobody thought to name.
const FORBIDDEN_MARKERS: &[&str] = &[
    "enigo",
    "xdotool",
    "CGEvent",
    "core_graphics",
    "core-graphics",
    "SendInput",
    "screencapture",
    "osascript",
    "winit",
    "unsafe",
];

fn crate_root() -> PathBuf {
    // Cargo runs a test binary with the cwd set to its own package root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Strip `//` line comments and `/* */` blocks, so prose *about* the forbidden list — of which this
/// crate has plenty, deliberately, because explaining why `enigo` is absent means naming it — is not
/// mistaken for a use of it.
///
/// Deliberately simple: it does not track string literals, so a `"//"` inside a string ends the line
/// early. That is acceptable because the only consumer is a substring search for crate names, and an
/// over-eager strip can only cause a false *pass* on a line that already contains no dependency.
fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut in_block = false;
    for line in src.lines() {
        let mut rest = line;
        loop {
            if in_block {
                match rest.find("*/") {
                    Some(end) => {
                        in_block = false;
                        rest = &rest[end + 2..];
                    }
                    None => break,
                }
            } else {
                let line_at = rest.find("//");
                let block_at = rest.find("/*");
                match (line_at, block_at) {
                    (Some(l), b) if b.is_none_or(|b| l < b) => {
                        out.push_str(&rest[..l]);
                        break;
                    }
                    (_, Some(b)) => {
                        out.push_str(&rest[..b]);
                        in_block = true;
                        rest = &rest[b + 2..];
                    }
                    _ => {
                        out.push_str(rest);
                        break;
                    }
                }
            }
        }
        out.push('\n');
    }
    out
}

#[test]
fn no_source_file_can_reach_the_operating_system() {
    // Every Rust file the crate ships, not just `src/`. Scanning one directory made the guarantee
    // bypassable by putting the offending code anywhere else — a build script, an example, a helper
    // module beside the manifest — which is exactly the kind of gap a ratchet exists to close.
    let mut files = Vec::new();
    for dir in ["src", "examples", "benches", "tests"] {
        rust_sources(&crate_root().join(dir), &mut files);
    }
    // `build.rs` and anything else sitting at the crate root.
    if let Ok(entries) = std::fs::read_dir(crate_root()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|e| e == "rs") {
                files.push(path);
            }
        }
    }
    // This test is itself one of the scanned files and necessarily names every forbidden marker, so
    // it must be excluded or it reports itself. The comment stripper handles prose; the `const`
    // array below is real code.
    files.retain(|p| p.file_name().is_some_and(|n| n != "leaf.rs"));
    assert!(
        !files.is_empty(),
        "expected to scan this crate's sources, found none — has the layout moved?"
    );

    let mut offenders = Vec::new();
    for path in &files {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let code = strip_comments(&text);
        for (n, line) in code.lines().enumerate() {
            for marker in FORBIDDEN_MARKERS {
                if line.contains(marker) {
                    offenders.push(format!(
                        "{}:{} — {}",
                        path.strip_prefix(crate_root()).unwrap_or(path).display(),
                        n + 1,
                        line.trim()
                    ));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "bevy_debugger_bevy must not be able to touch the OS, but {} line(s) reference something \
         that can.\n  {}\n\n\
         Screenshots here read an offscreen `Image`, and input is written into Bevy's own in-process \
         input message stream. Both guarantees are structural — they hold because nothing linked \
         into this crate is capable of synthesising an OS event. A capture that reads the window \
         needs the window raised, and a synthesised keystroke lands in whatever application actually \
         has focus. If you genuinely need one of these, it belongs in a different crate that a game \
         opts into knowingly.",
        offenders.len(),
        offenders.join("\n  ")
    );
}

#[test]
fn the_dependency_list_stays_closed() {
    let manifest = std::fs::read_to_string(crate_root().join("Cargo.toml"))
        .expect("bevy_debugger_bevy must have a Cargo.toml");

    // **Every** dependency table that can put code in a consumer's binary, not just the first one.
    // Reading only `[dependencies]` left `[target.'cfg(unix)'.dependencies]` — the natural home for a
    // platform-specific input crate — completely unchecked, so the boundary could be crossed by a
    // dependency the ratchet never looked at. `[dev-dependencies]` is deliberately excluded: it
    // cannot reach a game.
    let mut tables: Vec<&str> = Vec::new();
    for (i, section) in manifest.split("\n[").enumerate() {
        // `split` drops the delimiter, so re-attach it for the header test — except on the first
        // chunk, which is the text before any table.
        let header_end = section.find('\n').unwrap_or(section.len());
        let header = &section[..header_end];
        let is_dep_table = if i == 0 {
            false
        } else {
            header.starts_with("dependencies]") || header.ends_with(".dependencies]")
        };
        if is_dep_table {
            tables.push(&section[header_end..]);
        }
    }
    assert!(
        !tables.is_empty(),
        "bevy_debugger_bevy must declare a [dependencies] table"
    );

    for line in tables.concat().lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // `.` is a separator too, so an inherited dependency written as the dotted key
        // `serde.workspace = true` reads as `serde`. A crate name cannot contain a dot, so nothing
        // real is truncated. Continuation lines of a multi-line value (a feature array's entries, a
        // closing bracket) carry no `=` and are skipped below.
        let name = line.split(['=', ' ', '.']).next().unwrap_or("");
        if name.is_empty() || !line.contains('=') {
            continue;
        }
        assert!(
            ALLOWED_DEPS.contains(&name),
            "bevy_debugger_bevy took a new dependency: `{name}`.\n\n\
             The five allowed ({}) are allowed precisely because none of them can synthesise an \
             operating-system event — that is what makes \"injected input cannot reach your editor\" \
             a structural claim rather than a promise. Adding to this list means re-arguing it.",
            ALLOWED_DEPS.join(", "),
        );
    }
}
