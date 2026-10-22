//! The binary's command line answers what it is asked, and refuses what it does not know.
//!
//! `bevy-fairy --version` used to start the MCP server in stdio mode, wait for a client that
//! was never coming, and exit with "connection closed: initialized request". Any argument the
//! binary did not recognise did the same, so a typo looked like a server crash rather than a
//! typo. The loop's release check runs `bevy-fairy --version` to confirm the installed binary
//! matches the release, and it could not.

use std::process::{Command, Stdio};

fn bevy_fairy(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_bevy-fairy"))
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("running the bevy-fairy binary")
}

#[test]
fn version_prints_the_crate_version_and_exits_cleanly() {
    for flag in ["--version", "-V"] {
        let out = bevy_fairy(&[flag]);
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success(),
            "`bevy-fairy {flag}` must exit 0; got {:?}, stderr: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            stdout.trim(),
            format!("bevy-fairy {}", env!("CARGO_PKG_VERSION")),
            "`bevy-fairy {flag}` must print its name and version"
        );
    }
}

#[test]
fn an_unknown_argument_is_refused_by_name_rather_than_starting_the_server() {
    let out = bevy_fairy(&["--verison"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(2),
        "an unknown argument is a usage error (exit 2), not a server run; stderr: {stderr}"
    );
    assert!(
        stderr.contains("--verison"),
        "the error must name the argument it did not recognise; got: {stderr}"
    );
    assert!(
        stderr.contains("--version") && stderr.contains("--help"),
        "the error must list the arguments that do exist; got: {stderr}"
    );
}
