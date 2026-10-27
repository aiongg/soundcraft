use soundcraft_audio_io::AudioError;
use soundcraft_engine::EngineError;

use crate::automation_args::RootFlagError;

/// Why a subcommand failed: printed as `error: {e}`, exit 1.
#[derive(Debug, thiserror::Error)]
pub(crate) enum CliError {
    #[error(transparent)]
    Usage(#[from] UsageError),
    #[error(transparent)]
    RootFlag(#[from] RootFlagError),
    #[error(transparent)]
    Convert(#[from] ConvertError),
    #[error(transparent)]
    Engine(#[from] EngineError),
    #[error("unknown command `{0}`")]
    UnknownCommand(String),
    #[error("{id}: {source}")]
    Command { id: String, source: EngineError },
    #[error("{id}: bad JSON: {source}")]
    CmdJson { id: String, source: serde_json::Error },
    #[error("{path}:{line}: bad JSON: {source}")]
    ScriptJson { path: String, line: usize, source: serde_json::Error },
    #[error("{path}:{line}: {id}: {source}")]
    ScriptLine { path: String, line: usize, id: String, source: EngineError },
    #[error("{path}: {source}")]
    File { path: String, source: std::io::Error },
    #[error("{path}: {source}")]
    Audio { path: String, source: AudioError },
    /// The control channel of a running app failed ([`soundcraft_automation::Backend::call`] reports a string).
    #[error("{0}")]
    Remote(String),
    /// The automation roots were refused ([`soundcraft_engine::file_access::AutomationRoots::new`] reports a string).
    #[error("{0}")]
    Roots(String),
    #[error(transparent)]
    Serve(std::io::Error),
}

/// The command line was malformed. The hand parser reports these until clap takes it over.
#[derive(Debug, PartialEq, thiserror::Error)]
pub(crate) enum UsageError {
    #[error("unknown subcommand `{0}`")]
    UnknownSubcommand(String),
    #[error("unknown option {0}")]
    UnknownOption(String),
    #[error("{0} takes no value")]
    TakesNoValue(String),
    #[error("{0} needs a value")]
    NeedsValue(String),
    #[error("info FILE")]
    Info,
    #[error("convert IN OUT")]
    Convert,
    #[error("script FILE")]
    Script,
    #[error("describe ID")]
    Describe,
    #[error("app [--port P] METHOD [JSON]")]
    App,
    #[error("--connect needs a port or host:port")]
    ConnectNeedsPort,
    #[error("unknown mcp option `{0}`")]
    UnknownMcpOption(String),
    #[error(
        "--automation-read-root and --automation-write-root confine the in-process server; a running app is not confined by them, \
so drop --connect (or use --connect without the roots)"
    )]
    RootsWithConnect,
}

/// A `convert` option or output the tool refuses.
#[derive(Debug, PartialEq, thiserror::Error)]
pub(crate) enum ConvertError {
    #[error("--rate: expected a sample rate in Hz, got `{0}`")]
    Rate(String),
    #[error("--bit-depth: expected 16, 24 or 32f, got `{0}`")]
    BitDepth(String),
    #[error("--dither and --no-dither: choose one")]
    DitherConflict,
    #[error("{output}: cannot write `.{ext}` files (supported: {})", soundcraft_audio_io::ENCODE_EXTENSIONS)]
    OutputFormat { output: String, ext: String },
}
