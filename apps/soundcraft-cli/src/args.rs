use crate::error::UsageError;

pub(crate) fn arg_value(args: &[String], flag: &str) -> Option<String> {
    args.iter().enumerate().find_map(|(i, a)| {
        if a == flag { args.get(i + 1).cloned() } else { a.strip_prefix(flag).and_then(|rest| rest.strip_prefix('=')).map(str::to_owned) }
    })
}

/// Every option a subcommand accepts, with whether it carries a value.
pub(crate) type Options = [(&'static str, bool)];

/// A misspelled option, or one whose value is missing, reads as absent through `arg_value`, so the
/// subcommand would run without what was asked for and still report success. Reject both instead.
pub(crate) fn check_options(args: &[String], allowed: &Options) -> Result<(), UsageError> {
    let mut i = 0;
    while i < args.len() {
        let Some(name) = args[i].strip_prefix("--") else {
            i += 1;
            continue;
        };
        let (name, inline) = name.split_once('=').map_or((name, None), |(n, v)| (n, Some(v)));
        let flag = format!("--{name}");
        let Some(&(_, takes_value)) = allowed.iter().find(|(f, _)| f == &flag) else {
            return Err(UsageError::UnknownOption(flag));
        };
        match (takes_value, inline) {
            (false, Some(_)) => return Err(UsageError::TakesNoValue(flag)),
            (true, None) => {
                // A following option is the next flag, not this one's value.
                if args.get(i + 1).is_none_or(|v| v.starts_with("--")) {
                    return Err(UsageError::NeedsValue(flag));
                }
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }
    Ok(())
}

/// `--in`, `--demo`/`--sample`, `--bounce`, `--start`, `--end`, `--save` and `--inspect`, read by
/// `load_engine` and `finish` and so shared by `run` and `script`.
pub(crate) const ENGINE_OPTIONS: &Options = &[
    ("--in", true),
    ("--demo", false),
    ("--sample", false),
    ("--bounce", true),
    ("--start", true),
    ("--end", true),
    ("--save", true),
    ("--inspect", false),
];
