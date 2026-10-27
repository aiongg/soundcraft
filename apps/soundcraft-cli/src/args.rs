//! The command line, as clap types. A flag a subcommand doesn't take is a usage error (exit 2), so
//! a typo such as `--bounces` can't be silently ignored (#549), and so is a value flag without its
//! value: a following flag is never taken as the value.

use std::ffi::OsString;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::OnceLock;

use clap::builder::NonEmptyStringValueParser;
use clap::error::ErrorKind;
use clap::{ArgAction, Args, CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
use serde_json::{Value, json};
use soundcraft_automation::args::{AutomationArgs, ControlClientArgs, ControlTarget, blank_env_is_unset, control_target, name_env_vars};

/// soundcraft-cli: SoundCraft without a window
#[derive(Parser)]
#[command(name = "soundcraft-cli", version = long_version(), arg_required_else_help = true)]
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
}

/// Print an audio file's format as JSON, or a .scraft session's summary
#[derive(Args)]
pub(crate) struct InfoArgs {
    /// An audio file or a .scraft session
    pub(crate) file: PathBuf,
}

/// Convert an audio file: resample, change the bit depth, normalize
///
/// Dithers only when the output loses resolution, unless --dither or --no-dither says otherwise.
/// The output's extension picks its format.
#[derive(Args)]
pub(crate) struct ConvertArgs {
    /// The audio file to read
    #[arg(value_name = "IN")]
    pub(crate) input: PathBuf,
    /// The audio file to write
    #[arg(value_name = "OUT")]
    pub(crate) output: PathBuf,
    /// The output's bit depth
    #[arg(long, value_enum, value_name = "16|24|32f", default_value = "24")]
    pub(crate) bit_depth: BitDepthArg,
    /// Resample to this sample rate
    #[arg(long, value_name = "HZ")]
    pub(crate) rate: Option<NonZeroU32>,
    /// Normalize the peak to -0.1 dBFS
    #[arg(long)]
    pub(crate) normalize: bool,
    /// Always dither
    #[arg(long, conflicts_with = "no_dither")]
    pub(crate) dither: bool,
    /// Never dither
    #[arg(long)]
    pub(crate) no_dither: bool,
}

/// Where `run` and `script` start, and what they do with the result.
#[derive(Args)]
pub(crate) struct EngineArgs {
    /// Open this session first
    #[arg(long = "in", value_name = "S.scraft", conflicts_with = "demo")]
    pub(crate) input: Option<PathBuf>,
    /// Start from the demo session
    #[arg(long, visible_alias = "sample")]
    pub(crate) demo: bool,
    /// Bounce the mix to this file at the end; without --start and --end, the whole session
    #[arg(long, value_name = "OUT")]
    pub(crate) bounce: Option<PathBuf>,
    /// Where the bounce starts: seconds (12.5) or a time string
    #[arg(long, value_name = "T", allow_negative_numbers = true, requires = "bounce", value_parser = time_arg)]
    pub(crate) start: Option<TimeArg>,
    /// Where the bounce ends: seconds (12.5) or a time string
    #[arg(long, value_name = "T", allow_negative_numbers = true, requires = "bounce", value_parser = time_arg)]
    pub(crate) end: Option<TimeArg>,
    /// Save the session to this file at the end, replacing it
    #[arg(long, value_name = "S.scraft")]
    pub(crate) save: Option<PathBuf>,
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
    #[arg(long, value_name = "ID[=JSON]", action = ArgAction::Append, value_parser = cmd_spec)]
    pub(crate) cmd: Vec<CmdSpec>,
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
    pub(crate) file: PathBuf,
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
    #[arg(value_parser = NonEmptyStringValueParser::new())]
    pub(crate) filter: Option<String>,
}

/// Print one engine command's details as JSON
#[derive(Args)]
pub(crate) struct DescribeArgs {
    /// The command's id, such as file.bounce_mix
    #[arg(value_parser = NonEmptyStringValueParser::new())]
    pub(crate) id: String,
}

/// Call a running app's control channel
///
/// A dotted engine command id is shorthand for engine.execute; the control methods
/// (session.inspect, ui.screenshot, …) stay methods. Prints the reply as JSON.
#[derive(Args)]
pub(crate) struct AppArgs {
    #[command(flatten)]
    pub(crate) control: ControlClientArgs,
    /// A control method or engine command id
    #[arg(value_parser = NonEmptyStringValueParser::new())]
    pub(crate) method: String,
    /// Its parameters as JSON [default: {}]
    #[arg(value_parser = json_value)]
    pub(crate) json: Option<Value>,
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
    #[arg(long, value_name = "PORT", value_parser = control_target, conflicts_with_all = ["automation_read_root", "automation_write_root"])]
    pub(crate) connect: Option<ControlTarget>,
    /// Start the in-process engine from the demo session
    #[arg(long)]
    pub(crate) demo: bool,
    #[command(flatten)]
    pub(crate) automation: AutomationArgs,
}

/// Print the parity report (markdown)
#[derive(Args)]
pub(crate) struct ParityArgs {
    /// Write the report to this file instead
    #[arg(long, value_name = "PATH")]
    pub(crate) write: Option<PathBuf>,
}

/// The bit depths `convert` writes. `32` is the same as `32f`.
#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum BitDepthArg {
    #[value(name = "16")]
    Int16,
    #[value(name = "24")]
    Int24,
    #[value(name = "32f", alias = "32")]
    Float32,
}

impl From<BitDepthArg> for soundcraft_audio_io::BitDepth {
    fn from(depth: BitDepthArg) -> Self {
        match depth {
            BitDepthArg::Int16 => Self::Int16,
            BitDepthArg::Int24 => Self::Int24,
            BitDepthArg::Float32 => Self::Float32,
        }
    }
}

/// A bounce position: seconds, or a time string the engine reads.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TimeArg {
    Seconds(f64),
    Text(String),
}

impl TimeArg {
    /// The `start` / `end` parameter of `file.bounce_mix`.
    pub(crate) fn to_json(&self) -> Value {
        match self {
            Self::Seconds(s) => json!({"seconds": s}),
            Self::Text(t) => json!(t),
        }
    }
}

/// One `--cmd ID[=JSON]`: the command id and its parameters (`{}` when none are given).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CmdSpec {
    pub(crate) id: String,
    pub(crate) params: Value,
}

/// Why a value was refused: clap prints it after the flag and its value (exit 2).
#[derive(Debug, PartialEq, thiserror::Error)]
pub(crate) enum ValueError {
    #[error("expected seconds (12.5) or a time string, not an empty value")]
    EmptyTime,
    #[error("expected a command id, optionally followed by `=` and JSON parameters")]
    EmptyCommandId,
    #[error("bad JSON: {0}")]
    Json(String),
}

fn time_arg(s: &str) -> Result<TimeArg, ValueError> {
    if s.is_empty() {
        return Err(ValueError::EmptyTime);
    }
    Ok(s.parse::<f64>().map_or_else(|_| TimeArg::Text(s.to_owned()), TimeArg::Seconds))
}

fn json_value(s: &str) -> Result<Value, ValueError> {
    serde_json::from_str(s).map_err(|e| ValueError::Json(e.to_string()))
}

/// `ID` or `ID=JSON`; malformed JSON is an error, not `{}`.
fn cmd_spec(s: &str) -> Result<CmdSpec, ValueError> {
    let (id, params) = match s.split_once('=') {
        Some((id, p)) => (id, json_value(p)?),
        None => (s, json!({})),
    };
    if id.is_empty() {
        return Err(ValueError::EmptyCommandId);
    }
    Ok(CmdSpec { id: id.to_owned(), params })
}

/// `0.1.0 (linux x86_64)`: the version and the platform the binary was built for.
fn long_version() -> &'static str {
    static LONG: OnceLock<String> = OnceLock::new();
    LONG.get_or_init(|| format!("{} ({} {})", env!("CARGO_PKG_VERSION"), std::env::consts::OS, std::env::consts::ARCH))
}

/// Parse the arguments (without the binary name). clap reads them as `OsString`s: a folder need
/// not be valid Unicode, text must. A flag wins over its environment variable, unless that is
/// empty, and a bad value from one is a usage error naming the variable. A bare `--help` or `-h` is help even after a mistake on the
/// same line (`run --bounce --help`): clap reports the mistake first.
pub(crate) fn parse(args: &[OsString]) -> Result<Cli, clap::Error> {
    let mut cmd = Cli::command().mut_subcommands(|sub| sub.mut_args(blank_env_is_unset));
    let argv = std::iter::once(OsString::from("soundcraft-cli")).chain(args.iter().cloned());
    let matches = match cmd.try_get_matches_from_mut(argv) {
        Err(e) if e.kind() != ErrorKind::DisplayHelp => {
            let Some(flag) = args.iter().take_while(|a| *a != "--").find(|a| *a == "--help" || *a == "-h") else {
                return Err(name_env_vars(e, &cmd, args));
            };
            let help = [OsString::from("soundcraft-cli"), args.first().cloned().unwrap_or_default(), flag.clone()];
            return Err(cmd.try_get_matches_from_mut(help).err().unwrap_or(e));
        }
        matches => matches?,
    };
    Cli::from_arg_matches(&matches)
}
