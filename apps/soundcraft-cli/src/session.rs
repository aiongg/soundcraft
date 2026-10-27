use super::args::{EngineArgs, RunArgs, ScriptArgs};
use super::error::CliError;
use serde_json::{Value, json};
use soundcraft_engine::Engine;
use std::io::Write;

/// One stderr line per media file a session refers to but could not load (the exit code is unchanged).
pub(crate) fn warn_missing(err: &mut dyn Write, missing: &[String]) {
    for m in missing {
        outln!(err, "warning: missing media: {m}");
    }
}

fn load_engine(args: &EngineArgs, err: &mut dyn Write) -> Result<Engine, CliError> {
    if let Some(p) = &args.input {
        let mut e = Engine::default();
        let missing = soundcraft_engine::io::open_session(&mut e, p)?;
        warn_missing(err, &missing);
        Ok(e)
    } else if args.demo {
        Ok(soundcraft_engine::demo::demo_engine())
    } else {
        Ok(Engine::default())
    }
}

fn finish(e: &mut Engine, args: &EngineArgs, out: &mut dyn Write) -> Result<(), CliError> {
    if let Some(path) = &args.bounce {
        let mut p = json!({"path": path});
        if let Some(s) = &args.start {
            p["start"] = time_arg(s);
        }
        if let Some(s) = &args.end {
            p["end"] = time_arg(s);
        }
        // No range given: bounce the whole session, not whatever edit selection the commands left.
        if p.get("start").is_none() && p.get("end").is_none() {
            p["start"] = json!(0);
            p["end"] = json!(e.session().content_end().max(1));
        }
        let v = e.execute("file.bounce_mix", &p)?;
        outln!(out, "{v}");
    }
    if let Some(path) = &args.save {
        let v = e.execute("session.save", &json!({"path": path, "replace": true}))?;
        outln!(out, "{v}");
    }
    if args.inspect {
        outln!(out, "{}", serde_json::to_string_pretty(&soundcraft_engine::inspect::session(e, false)).unwrap_or_default());
    }
    Ok(())
}

/// `12.5` → seconds; anything else is passed through as a time string.
fn time_arg(s: &str) -> Value {
    s.parse::<f64>().map_or_else(|_| json!(s), |f| json!({"seconds": f}))
}

/// `ID` or `ID=JSON` → command id and params; malformed JSON is an error, not `{}`.
fn parse_cmd_spec(spec: &str) -> Result<(String, Value), CliError> {
    match spec.split_once('=') {
        Some((id, p)) => {
            serde_json::from_str::<Value>(p).map(|v| (id.to_string(), v)).map_err(|source| CliError::CmdJson { id: id.to_string(), source })
        }
        None => Ok((spec.to_string(), json!({}))),
    }
}

pub(crate) fn run_cmds(args: &RunArgs, out: &mut dyn Write, err: &mut dyn Write) -> Result<(), CliError> {
    let mut e = load_engine(&args.engine, err)?;
    for spec in &args.cmd {
        let (id, params) = parse_cmd_spec(spec)?;
        match e.execute(&id, &params) {
            Ok(v) => outln!(out, "{id}: {v}"),
            Err(source) => return Err(CliError::Command { id, source }),
        }
    }
    finish(&mut e, &args.engine, out)
}

pub(crate) fn script(args: &ScriptArgs, out: &mut dyn Write, err: &mut dyn Write) -> Result<(), CliError> {
    let path = &args.file;
    let text = std::fs::read_to_string(path).map_err(|source| CliError::File { path: path.clone(), source })?;
    let mut e = load_engine(&args.engine, err)?;
    let keep = args.keep_going;
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (id, p) = line.split_once(char::is_whitespace).unwrap_or((line, "{}"));
        let params = serde_json::from_str::<Value>(p.trim()).map_err(|source| CliError::ScriptJson { path: path.clone(), line: n + 1, source })?;
        match e.execute(id, &params) {
            Ok(v) => outln!(out, "{id}: {v}"),
            Err(source) => {
                let failure = CliError::ScriptLine { path: path.clone(), line: n + 1, id: id.to_owned(), source };
                if !keep {
                    return Err(failure);
                }
                outln!(err, "{failure}");
            }
        }
    }
    finish(&mut e, &args.engine, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_media_gets_one_warning_each() {
        let mut err = Vec::new();
        warn_missing(&mut err, &[]);
        assert!(err.is_empty());
        warn_missing(&mut err, &["a.wav".to_string(), "b.wav: decode failed".to_string()]);
        assert_eq!(String::from_utf8(err).unwrap(), "warning: missing media: a.wav\nwarning: missing media: b.wav: decode failed\n");
    }

    #[test]
    fn cmd_spec_parses_id_and_json() {
        assert_eq!(parse_cmd_spec("a.b").unwrap(), ("a.b".to_string(), json!({})));
        assert_eq!(parse_cmd_spec("a.b={\"x\":1}").unwrap(), ("a.b".to_string(), json!({"x": 1})));
    }

    #[test]
    fn cmd_spec_rejects_truncated_json() {
        let err = parse_cmd_spec("a.b={\"x\":").unwrap_err().to_string();
        assert!(err.starts_with("a.b: bad JSON"), "{err}");
    }
}
