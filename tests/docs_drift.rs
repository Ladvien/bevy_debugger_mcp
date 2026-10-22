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

/// No doc may tell the reader to add `RemotePlugin` to their game. The plugin owns it, and
/// Bevy rejects a duplicate plugin by name, so a game that adds it twice panics on start.
///
/// This started as a check on the README alone, and it found the same mistake in three other
/// files: the README's Quick Start, the README's developer workflow, and the macOS service
/// page. So it now walks every doc in the tree.
#[test]
fn no_doc_tells_the_reader_to_add_a_second_remote_plugin() {
    let plugin = read("crates/bevy_fairy/src/lib.rs");

    // The plugin registers these itself.
    for method in [
        "bevy_fairy/screenshot",
        "bevy_fairy/capture",
        "bevy_fairy/input",
        "bevy_fairy/commands",
        "bevy_fairy/scene",
        "bevy_fairy/quit",
        "bevy_fairy/status",
    ] {
        assert!(
            plugin.contains(&format!("\"{method}\"")),
            "the plugin is expected to register {method}"
        );
    }

    for (what, text) in doc_files() {
        assert_no_hand_registration(&what, &text);
        assert_no_remote_plugin(&what, &text);
    }
}

/// Every markdown file in the repository, by path, with its contents.
fn doc_files() -> Vec<(String, String)> {
    let mut files = Vec::new();
    for dir in ["." , "book", "docs"] {
        let path = root().join(dir);
        if !path.is_dir() {
            continue;
        }
        let mut stack = vec![path];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("reading a directory") {
                let entry = entry.expect("a directory entry");
                let path = entry.path();
                if path.is_dir() {
                    // `target/` and `.git/` hold no docs and are huge.
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
                    if name == "target" || name == ".git" {
                        continue;
                    }
                    stack.push(path);
                    continue;
                }
                if path.extension().and_then(|e| e.to_str()) != Some("md") {
                    continue;
                }
                let relative = path
                    .strip_prefix(root())
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned();
                let text = std::fs::read_to_string(&path).expect("reading a markdown file");
                files.push((relative, text));
            }
        }
    }
    files.sort();
    files
}

/// A registration is written as `.with_method("name", handler)`. No doc may tell the reader to do
/// that for a method the plugin owns.
fn assert_no_hand_registration(what: &str, text: &str) {
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(".with_method(\"") {
            let name = rest.split('"').next().unwrap_or_default();
            if name.starts_with("bevy_fairy/") {
                panic!(
                    "{what} tells the reader to register {name} by hand, but the plugin already \
                     registers it. Adding it again adds `RemotePlugin` twice, which Bevy rejects, so \
                     the game panics on start. Line: {trimmed}"
                );
            }
        }
    }
}

/// `RemotePlugin` is added by the plugin. A doc that tells the reader to add it is telling them
/// to add it twice.
fn assert_no_remote_plugin(what: &str, text: &str) {
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.contains("RemotePlugin") {
            continue;
        }
        // A line that says the plugin owns it, or that says never to add a second one, is the
        // doc telling the reader the right thing.
        let lower = trimmed.to_lowercase();
        let mentions_the_plugin = trimmed.contains("DebuggerPlugin");
        let warns = lower.contains("never")
            || lower.contains("not add")
            || lower.contains("do not add")
            || lower.contains("owns the");
        if mentions_the_plugin || warns {
            continue;
        }
        // A line that describes the mistake rather than instructing it -- a bug report, a
        // changelog entry, a backlog item -- is not telling the reader to do anything. Only a
        // line that looks like code or an instruction is.
        if !looks_like_an_instruction(trimmed) {
            continue;
        }
        panic!(
            "{what} tells the reader to add `RemotePlugin` themselves, but `DebuggerPlugin` \
             already adds it. Bevy rejects a duplicate plugin by name, so the game panics on \
             start. Line: {trimmed}"
        );
    }
}

/// A line that shows a plugin being added, rather than one that talks about the mistake.
fn looks_like_an_instruction(line: &str) -> bool {
    line.contains(".add_plugins(") || line.contains("add_plugins(")
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

/// The changelog's migration table names the fields the tools take. If a field is renamed again
/// and the table is not updated, an agent that read the changelog will send a field the tool
/// does not have -- which is exactly the bug the table exists to prevent.
#[test]
fn the_changelog_migration_table_names_the_fields_the_tools_take() {
    let changelog = read("CHANGELOG.md");
    let tools = read("src/mcp_tools.rs");

    // The table row for each tool: `| `tool` | before | after |`.
    for tool in ["experiment", "hypothesis", "detect_anomaly", "stress_test"] {
        let row = changelog
            .lines()
            .find(|line| {
                let trimmed = line.trim();
                trimmed.starts_with(&format!("| `{tool}` |"))
            })
            .unwrap_or_else(|| panic!("CHANGELOG.md has no migration row for `{tool}`"));

        // The "after" cell is the last one.
        let after = row
            .rsplit('|')
            .nth(1)
            .expect("the row has cells")
            .trim();
        let after = after.trim_start_matches('`').trim_end_matches('`');

        // Every backticked name in the cell must be a field of the tool's request struct.
        let struct_name = match tool {
            "experiment" => "ExperimentRequest",
            "hypothesis" => "HypothesisRequest",
            "detect_anomaly" => "AnomalyRequest",
            "stress_test" => "StressTestRequest",
            _ => unreachable!("the list above is exhaustive"),
        };
        let start = tools
            .find(&format!("pub struct {struct_name} {{"))
            .unwrap_or_else(|| panic!("{struct_name} is not in src/mcp_tools.rs"));
        let body = &tools[start..];
        let end = body.find("\n}").expect("the struct is closed");
        let body = &body[..end];

        for field in after.split(',').map(|f| f.trim().trim_matches('`')) {
            if field.is_empty() || field == "..." {
                continue;
            }
            assert!(
                body.contains(&format!("pub {field}:")),
                "CHANGELOG.md says `{tool}` takes `{field}`, but {struct_name} has no such field. \
                 The migration table must name the fields the tool actually takes."
            );
        }
    }
}
