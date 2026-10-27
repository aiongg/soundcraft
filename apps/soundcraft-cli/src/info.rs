use super::args::InfoArgs;
use super::error::CliError;
use super::session::warn_missing;
use serde_json::json;
use soundcraft_engine::Engine;
use std::io::Write;

pub(crate) fn info(args: &InfoArgs, out: &mut dyn Write, err: &mut dyn Write) -> Result<(), CliError> {
    let path = &args.file;
    if path.ends_with(".scraft") {
        let mut e = Engine::default();
        let missing = soundcraft_engine::io::open_session(&mut e, path)?;
        warn_missing(err, &missing);
        outln!(out, "{}", soundcraft_engine::inspect::session_text(&e));
        return Ok(());
    }
    let bytes = std::fs::read(path).map_err(|source| CliError::File { path: path.clone(), source })?;
    let ext = std::path::Path::new(path).extension().and_then(std::ffi::OsStr::to_str);
    let i = soundcraft_audio_io::probe(&bytes, ext).map_err(|source| CliError::Audio { path: path.clone(), source })?;
    outln!(out, "{}", serde_json::to_string_pretty(&json!({"format": i.format, "sample_format": i.sample_format, "sample_rate": i.sample_rate, "channels": i.channels, "frames": i.frames, "seconds": i.duration_secs(), "bwf": i.bwf})).unwrap_or_default());
    Ok(())
}
