//! **The crate boundary, enforced — and here the boundary is the OS.**
//!
//! Every other dependency ratchet in this workspace asks "is this crate still engine-free?". This one
//! cannot: the plugin's whole job is to live inside a Bevy `App`. What it must never do is reach the
//! machine the game is running on.
//!
//! That property is the reason this crate exists at all. Driving the OS keyboard sends keystrokes to
//! whatever application actually has focus, which may be somebody's editor, so input must stay in the
//! process. Window capture reads the game's own swapchain and never raises or focuses the window, but
//! a covered or minimised window can return a stale frame; measured on the game this was built for,
//! **7,188 distinct colours** came back with the window focused and **1** with another app in front.
//! Offscreen capture avoids that entirely, so it is the default.
//!
//! So the guarantee is structural rather than careful. `bevy_fairy/screenshot` reads an `Image` a
//! camera rendered to (offscreen) or the primary window's swapchain (window), and `bevy_fairy/input`
//! writes into the same in-process `Messages` buffer `bevy_winit` appends to. None of these can leak
//! outside the process, *because there is nothing linked here that could do it*. The input half got
//! **stronger** when it moved from `ButtonInput` to the message stream: a `Messages<KeyboardInput>`
//! buffer is an ECS resource, and reaching a real event loop would take a dependency this file forbids.
//!
//! ## What changed when the two halves became one crate
//!
//! The plugin used to be its own package, and the guarantee came from its *dependency list* being
//! closed: five crates, none of which could synthesise an event. That argument no longer works the
//! same way, because this crate also contains the MCP server, and the server needs `tokio`,
//! `reqwest`, `axum` and the rest.
//!
//! So the guarantee is now stated in terms of **features** rather than packages. The plugin is behind
//! the `plugin` feature; the server is behind the `server` feature; and the two are mutually
//! exclusive in a game, which depends on this crate with `--no-default-features --features plugin`.
//! Cargo resolves dependencies per feature, so a game that takes the plugin compiles the plugin's
//! closure and *none* of the server's.
//!
//! That makes the check stronger than the one it replaces, because it is now a check on the resolved
//! dependency graph rather than on a list someone maintains:
//!
//! - `the_plugin_closure_contains_nothing_that_can_synthesise_an_event` runs `cargo tree` for the
//!   plugin's feature set and fails if anything in it can.
//! - `the_plugin_direct_dependencies_stay_small` pins the plugin's *direct* dependencies, so a new one
//!   cannot be added without re-arguing why it is safe.
//! - `no_source_file_can_reach_the_operating_system` still scans the plugin's sources for the
//!   forbidden names, including `unsafe`.
//!
//! Widening any of these lists is a design decision, so it should cost a deliberate edit in this file
//! rather than passing silently in a build.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The dependencies the plugin is allowed to have directly.
///
/// `bevy` is the plugin and the protocol; `serde`/`serde_json` parse method params; `image` encodes
/// the captured PNG. None of the four can synthesise an operating-system event, and that is the entire
/// argument for why injected input cannot escape the game process.
const ALLOWED_DEPS: &[&str] = &["bevy", "serde", "serde_json", "image"];

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

/// Crates that can synthesise an operating-system event, or read which window has focus.
///
/// This is the list the resolved dependency graph is checked against. It is a superset of
/// `FORBIDDEN_MARKERS`, because the markers are searched for in source text and this one is searched
/// for in resolved package names.
const EVENT_SYNTHESIS_CRATES: &[&str] = &[
    "enigo",
    "xdotool",
    "rdev",
    "device_query",
    "autopilot",
    "winit",
    "core-graphics",
    "coregraphics",
    "windows",
    "winapi",
    "objc",
    "objc2",
];

fn crate_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
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
    let mut chars = src.chars().peekable();
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut in_string = false;
    let mut in_char = false;

    while let Some(c) = chars.next() {
        if in_line_comment {
            if c == '\n' {
                in_line_comment = false;
                out.push(c);
            }
            continue;
        }
        if in_block_comment {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block_comment = false;
            }
            continue;
        }
        if in_string {
            out.push(c);
            if c == '\\' {
                if let Some(next) = chars.next() {
                    out.push(next);
                }
                continue;
            }
            if c == '"' {
                in_string = false;
            }
            continue;
        }
        if in_char {
            out.push(c);
            if c == '\\' {
                if let Some(next) = chars.next() {
                    out.push(next);
                }
                continue;
            }
            if c == '\'' {
                in_char = false;
            }
            continue;
        }
        match c {
            '/' if chars.peek() == Some(&'/') => {
                chars.next();
                in_line_comment = true;
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                in_block_comment = true;
            }
            '"' => {
                in_string = true;
                out.push(c);
            }
            '\'' => {
                in_char = true;
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

#[test]
fn no_source_file_can_reach_the_operating_system() {
    // Every Rust file the plugin ships, not just `src/`. Scanning one directory made the guarantee
    // bypassable by putting the offending code anywhere else — a build script, an example, a helper
    // module beside the manifest — which is exactly the kind of gap a ratchet exists to close.
    let mut files = Vec::new();
    for dir in ["src/plugin", "examples"] {
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
    files.retain(|p| p.file_name().is_some_and(|n| n != "plugin_leaf.rs"));
    assert!(
        !files.is_empty(),
        "expected to scan the plugin's sources, found none — has the layout moved?"
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
        "the plugin must not be able to touch the OS, but {} line(s) reference something \
         that can.\n  {}\n\n\
         Screenshots here read an offscreen `Image`, and input is written into Bevy's own in-process \
         input message stream. Both guarantees are structural — they hold because nothing linked \
         into the plugin is capable of synthesising an OS event. A capture that reads the window \
         needs the window raised, and a synthesised keystroke lands in whatever application actually \
         has focus. If you genuinely need one of these, it belongs in a different crate that a game \
         opts into knowingly.",
        offenders.len(),
        offenders.join("\n  ")
    );
}

/// The plugin's direct dependencies. The server's dependencies live in the same manifest, so this
/// checks the plugin's own closure rather than the manifest as a whole.
#[test]
fn the_plugin_direct_dependencies_stay_small() {
    let tree = cargo_tree_for_the_plugin(Some(1));
    let direct = tree
        .lines()
        .skip(1) // the root
        .filter_map(|line| {
            let name = line
                .trim()
                .trim_start_matches(['├', '└', '│', '─', ' '])
                .split_whitespace()
                .next()?;
            Some(name.to_string())
        })
        .collect::<Vec<_>>();

    assert!(
        !direct.is_empty(),
        "cargo tree produced no direct dependencies — has the plugin's feature set changed?"
    );

    for name in &direct {
        assert!(
            ALLOWED_DEPS.contains(&name.as_str()),
            "the plugin took a new direct dependency: `{name}`.\n\n\
             The allowed ones ({}) are allowed precisely because none of them can synthesise an \
             operating-system event — that is what makes \"injected input cannot reach your editor\" \
             a structural claim rather than a promise. Adding to this list means re-arguing it.",
            ALLOWED_DEPS.join(", "),
        );
    }
}

/// The resolved dependency graph a game compiles when it takes the plugin. Nothing in it may be able
/// to synthesise an operating-system event.
#[test]
fn the_plugin_closure_contains_nothing_that_can_synthesise_an_event() {
    let tree = cargo_tree_for_the_plugin(None);

    let mut offenders = Vec::new();
    for line in tree.lines() {
        let name = line
            .trim()
            .trim_start_matches(['├', '└', '│', '─', ' '])
            .split_whitespace()
            .next()
            .unwrap_or_default();
        for crate_name in EVENT_SYNTHESIS_CRATES {
            if name.eq_ignore_ascii_case(crate_name) {
                offenders.push(name.to_string());
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "the plugin's dependency closure contains {} crate(s) that can synthesise an operating-system \
         event or read which window has focus: {}.\n\n\
         This is the check that keeps \"injected input cannot reach your editor\" structural. A game \
         that depends on this crate with `--no-default-features --features plugin` compiles \
         everything in this closure, so anything in it that can synthesise an event is a hole in the \
         guarantee. The server's dependencies are behind the `server` feature and are not in this \
         closure — check that the feature split has not been undone.",
        offenders.len(),
        offenders.join(", ")
    );
}

/// `cargo tree` for the plugin's feature set: no default features, and the plugin.
///
/// `depth` is the tree's depth; `None` means the whole closure.
fn cargo_tree_for_the_plugin(depth: Option<usize>) -> String {
    let depth_arg = depth.map(|d| d.to_string());
    let mut args = vec![
        "tree".to_string(),
        "--no-default-features".to_string(),
        "--features".to_string(),
        "plugin".to_string(),
        "--edges".to_string(),
        "normal".to_string(),
    ];
    if let Some(depth) = depth_arg {
        args.push("--depth".to_string());
        args.push(depth);
    }
    let output = Command::new(env!("CARGO"))
        .args(&args)
        .current_dir(crate_root())
        .output()
        .expect("running cargo tree");

    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8_lossy(&output.stdout).into_owned()
}
