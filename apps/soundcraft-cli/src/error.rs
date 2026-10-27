use std::path::{Path, PathBuf};

use soundcraft_audio_io::AudioError;
use soundcraft_engine::EngineError;

/// Why a subcommand failed: printed as `error: {e}`, exit 1.
#[derive(Debug, thiserror::Error)]
pub(crate) enum CliError {
    #[error(transparent)]
    Convert(#[from] ConvertError),
    #[error(transparent)]
    Engine(#[from] EngineError),
    #[error("unknown command `{0}`")]
    UnknownCommand(String),
    #[error("{id}: {source}")]
    Command { id: String, source: EngineError },
    #[error("{}:{line}: bad JSON: {source}", path.display())]
    ScriptJson { path: PathBuf, line: usize, source: serde_json::Error },
    #[error("{}:{line}: {id}: {source}", path.display())]
    ScriptLine { path: PathBuf, line: usize, id: String, source: EngineError },
    #[error("{}: {source}", path.display())]
    File { path: PathBuf, source: std::io::Error },
    #[error("{}: {source}", path.display())]
    Audio { path: PathBuf, source: AudioError },
    /// The engine takes session and bounce paths as text.
    #[error("{}: the path is not valid Unicode", .0.display())]
    NonUnicodePath(PathBuf),
    /// The control channel of a running app failed ([`soundcraft_automation::Backend::call`] reports a string).
    #[error("{0}")]
    Remote(String),
    /// The automation roots were refused ([`soundcraft_engine::file_access::AutomationRoots::new`] reports a string).
    #[error("{0}")]
    Roots(String),
    #[error(transparent)]
    Serve(std::io::Error),
}

/// A path as the text the engine takes.
pub(crate) fn utf8(path: &Path) -> Result<&str, CliError> {
    path.to_str().ok_or_else(|| CliError::NonUnicodePath(path.to_owned()))
}

/// A `convert` option or output the tool refuses.
#[derive(Debug, PartialEq, thiserror::Error)]
pub(crate) enum ConvertError {
    #[error("{}: cannot write `.{ext}` files (supported: {})", output.display(), soundcraft_audio_io::ENCODE_EXTENSIONS)]
    OutputFormat { output: PathBuf, ext: String },
}
