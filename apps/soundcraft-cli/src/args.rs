//! The command line, as clap types. A flag a subcommand doesn't take is a usage error (exit 2), so
//! a typo such as `--bounces` can't be silently ignored (#549), and so is a value flag without its
//! value: a following flag is never taken as the value.

use std::ffi::OsString;
use std::path::PathBuf;

use clap::error::ErrorKind;
use clap::{ArgAction, Args, CommandFactory, FromArgMatches, Parser, Subcommand};

/// soundcraft-cli: SoundCraft without a window
#[derive(Parser)]
#[command(name = "soundcraft-cli", version, arg_required_else_help = true, args_override_self = true)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand)]
pub(crate) enum Command {
    Info(InfoArgs),
    Convert(ConvertArgs),
    Run(RunArgs),
    Script(ScriptArgs),
    Commands(CommandsArgs),
    Describe(DescribeArgs),
    App(AppArgs),
    Mcp(McpArgs),
    Parity(ParityArgs),
    /// List the built-in plugins
    Plugins,
    /// Print the version
    #[command(hide = true)]
    Version,
}

/// Print an audio file's format as JSON, or a .scraft session's summary
#[derive(Args)]
pub(crate) struct InfoArgs {
    /// An audio file or a .scraft session
    pub(crate) file: String,
}

/// Convert an audio file: resample, change the bit depth, normalize
///
/// Dithers only when the output loses resolution, unless --dither or --no-dither says otherwise.
/// The output's extension picks its format.
#[derive(Args)]
pub(crate) struct ConvertArgs {
    /// The audio file to read
    #[arg(value_name = "IN")]
    pub(crate) input: String,
    /// The audio file to write
    #[arg(value_name = "OUT")]
    pub(crate) output: String,
    /// The output's bit depth: 16, 24 (the default) or 32f
    #[arg(long, value_name = "16|24|32f")]
    pub(crate) bit_depth: Option<String>,
    /// Resample to this sample rate
    #[arg(long, value_name = "HZ")]
    pub(crate) rate: Option<String>,
    /// Normalize the peak to -0.1 dBFS
    #[arg(long)]
    pub(crate) normalize: bool,
    /// Always dither
    #[arg(long)]
    pub(crate) dither: bool,
    /// Never dither
    #[arg(long)]
    pub(crate) no_dither: bool,
}

/// Where `run` and `script` start, and what they do with the result.
#[derive(Args)]
pub(crate) struct EngineArgs {
    /// Open this session first
    #[arg(long = "in", value_name = "S.scraft")]
    pub(crate) input: Option<String>,
    /// Start from the demo session (ignored with --in)
    #[arg(long, visible_alias = "sample")]
    pub(crate) demo: bool,
    /// Bounce the mix to this file at the end; without --start and --end, the whole session
    #[arg(long, value_name = "OUT")]
    pub(crate) bounce: Option<String>,
    /// Where the bounce starts: seconds (12.5) or a time string
    #[arg(long, value_name = "T")]
    pub(crate) start: Option<String>,
    /// Where the bounce ends: seconds (12.5) or a time string
    #[arg(long, value_name = "T")]
    pub(crate) end: Option<String>,
    /// Save the session to this file at the end, replacing it
    #[arg(long, value_name = "S.scraft")]
    pub(crate) save: Option<String>,
    /// Print the session as JSON at the end
    #[arg(long)]
    pub(crate) inspect: bool,
}

/// Run engine commands on a new, demo or saved session
///
/// Prints each command's JSON result, then bounces, saves and inspects, in that order.
#[derive(Args)]
pub(crate) struct RunArgs {
    /// An engine command, with its JSON parameters after `=` (default {}); repeat for several, run
    /// in order
    #[arg(long, value_name = "ID[=JSON]", action = ArgAction::Append)]
    pub(crate) cmd: Vec<String>,
    #[command(flatten)]
    pub(crate) engine: EngineArgs,
}

/// Run a file of engine commands, one `ID JSON` per line
///
/// Blank lines and lines starting with # are skipped. A failing line stops the script unless
/// --keep-going is given.
#[derive(Args)]
pub(crate) struct ScriptArgs {
    /// The script to run
    pub(crate) file: String,
    /// Report a failing line and go on with the next
    #[arg(long)]
    pub(crate) keep_going: bool,
    #[command(flatten)]
    pub(crate) engine: EngineArgs,
}

/// List the engine commands
#[derive(Args)]
pub(crate) struct CommandsArgs {
    /// Only the commands whose id, or label in any case, contains this text
    pub(crate) filter: Option<String>,
}

/// Print one engine command's details as JSON
#[derive(Args)]
pub(crate) struct DescribeArgs {
    /// The command's id, such as file.bounce_mix
    pub(crate) id: String,
}

/// Call a running app's control channel
///
/// A dotted engine command id is shorthand for engine.execute; the control methods
/// (session.inspect, ui.screenshot, …) stay methods. Prints the reply as JSON.
#[derive(Args)]
pub(crate) struct AppArgs {
    /// The app's control port or host:port [default: $SOUNDCRAFT_CONTROL_PORT, else 7979]
    #[arg(long, value_name = "P")]
    pub(crate) port: Option<String>,
    /// A control method or engine command id
    pub(crate) method: String,
    /// Its parameters as JSON [default: {}]
    pub(crate) json: Option<String>,
}

/// Serve MCP on stdin and stdout
///
/// The automation roots confine every file the agent's commands read (open, import, presets) and
/// write (save, bounce, export) to those folders, links followed. Each root flag may be given more
/// than once, a path inside any of its folders being allowed, and a kind left out grants none of
/// its access. The roots confine the in-process server, so they conflict with --connect.
#[derive(Args)]
pub(crate) struct McpArgs {
    /// Serve a running app's control channel at this port or host:port instead of an in-process
    /// engine
    #[arg(long, value_name = "PORT")]
    pub(crate) connect: Option<String>,
    /// Start the in-process engine from the demo session
    #[arg(long)]
    pub(crate) demo: bool,
    /// A folder the agent's commands may read from; repeat for several
    #[arg(long, value_name = "DIR", action = ArgAction::Append)]
    pub(crate) automation_read_root: Vec<PathBuf>,
    /// A folder the agent's commands may write to; repeat for several
    #[arg(long, value_name = "DIR", action = ArgAction::Append)]
    pub(crate) automation_write_root: Vec<PathBuf>,
}

/// Print the parity report (markdown)
#[derive(Args)]
pub(crate) struct ParityArgs {
    /// Write the report to this file instead
    #[arg(long, value_name = "PATH")]
    pub(crate) write: Option<String>,
}

/// Parse the arguments (without the binary name). clap reads them as `OsString`s: a folder need
/// not be valid Unicode, text must. A bare `--help` or `-h` is help even after a mistake on the
/// same line (`run --bounce --help`): clap reports the mistake first.
pub(crate) fn parse(args: &[OsString]) -> Result<Cli, clap::Error> {
    let mut cmd = Cli::command();
    let argv = std::iter::once(OsString::from("soundcraft-cli")).chain(args.iter().cloned());
    let matches = match cmd.try_get_matches_from_mut(argv) {
        Err(e) if e.kind() != ErrorKind::DisplayHelp => {
            let Some(flag) = args.iter().take_while(|a| *a != "--").find(|a| *a == "--help" || *a == "-h") else { return Err(e) };
            let help = [OsString::from("soundcraft-cli"), args.first().cloned().unwrap_or_default(), flag.clone()];
            return Err(cmd.try_get_matches_from_mut(help).err().unwrap_or(e));
        }
        matches => matches?,
    };
    Cli::from_arg_matches(&matches)
}
