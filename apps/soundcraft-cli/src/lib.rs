//! `soundcraft-cli`: SoundCraft without a window.
//!
//! ```text
//! soundcraft-cli info FILE                       audio file or .scraft summary
//! soundcraft-cli convert IN OUT [--bit-depth 16|24|32f] [--rate HZ] [--normalize] [--dither | --no-dither]
//!                                                 dithers only when the output loses resolution, unless told
//! soundcraft-cli run [--in S.scraft | --demo] [--cmd ID[=JSON]]… [--bounce OUT] [--start T --end T] [--save S.scraft] [--inspect]
//! soundcraft-cli script FILE [--in S | --demo] [--save S] [--bounce OUT [--start T --end T]]   (one `ID JSON` per line;
//!                                                 without --start/--end, --bounce covers the whole session)
//! soundcraft-cli commands [FILTER]               list commands
//! soundcraft-cli describe ID                     one command's details
//! soundcraft-cli app [--port P] METHOD [JSON]     call a running app's control channel
//! soundcraft-cli mcp [--connect PORT] [--demo] [--automation-read-root DIR]... [--automation-write-root DIR]...
//!                                                 MCP server on stdio. The roots confine every file the
//!                                                 agent's commands read (open, import, presets) and write
//!                                                 (save, bounce, export) to those folders, links followed;
//!                                                 each flag may be given more than once, a path inside any
//!                                                 of its folders being allowed, and a kind left out grants
//!                                                 none of its access. They run the in-process server, so
//!                                                 they conflict with --connect.
//! soundcraft-cli parity [--write PATH]            parity report (markdown)
//! soundcraft-cli plugins                          list built-in plugins
//! ```
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod automation_args;
#[macro_use]
mod output;

mod app;
mod args;
mod catalog;
mod convert;
mod error;
mod info;
mod mcp;
mod session;

use std::io::Write;

use app::app;
use catalog::{commands, describe, parity};
use convert::convert;
use error::{CliError, UsageError};
use info::info;
use mcp::mcp;
use session::{run_cmds, script};

/// The usage block of the module doc comment, up to its closing code fence.
fn help_text() -> String {
    include_str!("lib.rs")
        .lines()
        .skip(3)
        .take_while(|l| l.trim_end() != "//! ```")
        .map(|l| l.trim_start_matches("//! "))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Run the CLI on its arguments (without the program name), writing results to `out` and
/// diagnostics to `err`. Returns the process exit code: 0 on success, 1 on failure. Plugin hosts
/// are shut down by the caller once the process is done with every engine (see `main`).
pub fn run<I, T>(args: I, out: &mut dyn Write, err: &mut dyn Write) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<String>,
{
    let args: Vec<String> = args.into_iter().map(Into::into).collect();
    let Some((cmd, rest)) = args.split_first() else {
        outln!(err, "usage: soundcraft-cli <info|convert|run|script|commands|describe|app|mcp|parity|plugins> …  (see --help)");
        return 1;
    };
    match dispatch(cmd, rest, out, err) {
        Ok(()) => 0,
        Err(e) => {
            outln!(err, "error: {e}");
            1
        }
    }
}

fn dispatch(cmd: &str, rest: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Result<(), CliError> {
    match cmd {
        "--version" | "-V" | "version" => outln!(out, "SoundCraft CLI {}", env!("CARGO_PKG_VERSION")),
        "--help" | "-h" | "help" => outln!(out, "{}", help_text()),
        "info" => info(rest, out, err)?,
        "convert" => convert(rest, out)?,
        "run" => run_cmds(rest, out, err)?,
        "script" => script(rest, out, err)?,
        "commands" => commands(rest, out),
        "describe" => describe(rest, out)?,
        "app" => app(rest, out)?,
        "mcp" => mcp(rest, err)?,
        "parity" => parity(rest, out)?,
        "plugins" => {
            for p in soundcraft_dsp::plugins() {
                outln!(out, "{:<20} {:<22} {:?}{}", p.id, p.name, p.category, if p.is_instrument { " (instrument)" } else { "" });
            }
        }
        other => return Err(UsageError::UnknownSubcommand(other.to_owned()).into()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_text_is_only_the_usage_block() {
        let help = help_text();
        assert!(help.starts_with("soundcraft-cli info FILE"));
        assert!(help.ends_with("soundcraft-cli plugins                          list built-in plugins"));
        assert!(help.contains("--automation-read-root DIR") && help.contains("--automation-write-root DIR"));
        assert!(!help.contains("```"));
        assert!(!help.contains("#![") && !help.contains("use serde_json"));
    }
}
