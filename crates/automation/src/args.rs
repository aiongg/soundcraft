//! The control and automation launch options, as clap types, so each flag and its environment
//! variable are declared once for `soundcraft-cli` and the desktop app. A flag wins over its
//! variable.
//!
//! Parse with [`blank_env_is_unset`] applied to the arguments, so an empty variable counts as unset,
//! and pass a parse error through [`name_env_vars`], so a bad value names the variable it came from.
//! [`conflict`] reports two arguments that can't go together, each named as it was given, for a
//! conflict that depends on the mode, which clap's `conflicts_with` can't express.

use std::ffi::OsString;
use std::path::PathBuf;

use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::parser::ValueSource;
use clap::{Arg, ArgAction, ArgMatches, Args, Command};

/// The running app a client calls (`soundcraft-cli app`).
#[derive(Args)]
pub struct ControlClientArgs {
    /// The app's control port or host:port
    #[arg(long, value_name = "P", env = "SOUNDCRAFT_CONTROL_PORT", default_value = "7979", value_parser = control_target)]
    pub port: ControlTarget,
}

/// The control server the desktop app starts. The same variable as [`ControlClientArgs`], so one
/// export connects the two.
#[derive(Args)]
pub struct ControlServerArgs {
    /// Start the localhost JSON-lines control server on this port (0 picks a free port and prints
    /// it)
    // allow_negative_numbers makes `--control -1` a bad port rather than an unexpected argument.
    #[arg(long, value_name = "PORT", env = "SOUNDCRAFT_CONTROL_PORT", value_parser = control_port, allow_negative_numbers = true)]
    pub control: Option<ControlPort>,
}

/// The folders automation may touch. Once any root is given, files are confined to the roots, and
/// a kind left out grants none of its access. No variable stands in for them: one left exported
/// must not change what an agent may read or write.
#[derive(Args)]
pub struct AutomationArgs {
    /// A folder the agent's commands may read from; repeat for several
    #[arg(long, value_name = "DIR", action = ArgAction::Append)]
    pub automation_read_root: Vec<PathBuf>,
    /// A folder the agent's commands may write to; repeat for several
    #[arg(long, value_name = "DIR", action = ArgAction::Append)]
    pub automation_write_root: Vec<PathBuf>,
}

/// Where a running app's control channel listens: a port, or `host:port`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlTarget(String);

impl ControlTarget {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A [`ControlTarget`], for clap's `value_parser`: `soundcraft-cli app --port` and `mcp --connect`,
/// and SOUNDCRAFT_CONTROL_PORT, which may carry spaces.
pub fn control_target(s: &str) -> Result<ControlTarget, NotATarget> {
    let s = s.trim();
    host_and_port(s)?;
    Ok(ControlTarget(s.to_owned()))
}

/// `PORT` or `HOST:PORT`, split. An IPv6 host goes in brackets (`[::1]:7979`), as a socket address
/// is written: in `::1:7979` the host and port can't be told apart.
fn host_and_port(s: &str) -> Result<(Option<&str>, u16), NotATarget> {
    let (host, port) = match s.rsplit_once(':') {
        Some(("", _)) => return Err(NotATarget),
        Some((host, _)) if host.contains(':') && !(host.starts_with('[') && host.ends_with(']')) => return Err(NotATarget),
        Some((host, port)) => (Some(host), port),
        None => (None, s),
    };
    Ok((host, port.parse().map_err(|_| NotATarget)?))
}

/// A [`control_target`] or [`control_port`] value that is neither a port nor `host:port`.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("expected a port, 0-65535, or host:port")]
pub struct NotATarget;

/// A desktop control server port: a port, or `host:port` as `soundcraft-cli app` takes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControlPort {
    /// A port, or the port of a loopback `host:port`: the control server listens on 127.0.0.1.
    Local(u16),
    /// A `host:port` naming another host, which the control server can't listen on.
    Remote(String),
}

/// The hosts a [`ControlPort::Local`] may name: the control server listens on 127.0.0.1 only.
const LOOPBACK_HOSTS: [&str; 3] = ["127.0.0.1", "localhost", "[::1]"];

/// A [`ControlPort`], for clap's `value_parser`: `soundcraft --control` and
/// SOUNDCRAFT_CONTROL_PORT, which may carry spaces. The variable may hold the `host:port`
/// `soundcraft-cli app` takes, so one export serves both: a loopback host is dropped, and the app
/// decides what another host means.
pub fn control_port(s: &str) -> Result<ControlPort, NotATarget> {
    let s = s.trim();
    Ok(match host_and_port(s)? {
        (Some(host), _) if !LOOPBACK_HOSTS.contains(&host) => ControlPort::Remote(s.to_owned()),
        (_, port) => ControlPort::Local(port),
    })
}

/// For `Command::mut_args`: an empty or blank variable counts as unset, so `SOUNDCRAFT_CONTROL_PORT=`
/// turns a fallback off without `unset`. clap would pass the empty value to the parser.
pub fn blank_env_is_unset(arg: Arg) -> Arg {
    let blank = arg.get_env().and_then(std::env::var_os).is_some_and(|v| v.to_str().is_some_and(|v| v.trim().is_empty()));
    if blank { arg.env(None::<&str>) } else { arg }
}

/// clap names the flag when a value from its variable is bad or conflicts with another, as if the
/// flag had been given: name the variable instead when the flag isn't among `args` (without the
/// binary name; a first one naming a subcommand selects whose arguments to look in).
pub fn name_env_vars(mut error: clap::Error, cmd: &Command, args: &[OsString]) -> clap::Error {
    // `--flag value` or `--flag=value`.
    let given =
        |long: &str| args.iter().filter_map(|a| a.to_str()?.strip_prefix("--")?.strip_prefix(long)).any(|r| r.is_empty() || r.starts_with('='));
    let sub = args.first().and_then(|name| cmd.find_subcommand(name)).unwrap_or(cmd);
    for kind in [ContextKind::InvalidArg, ContextKind::PriorArg] {
        let Some(ContextValue::String(shown)) = error.get(kind) else { continue };
        let Some(arg) = sub.get_arguments().find(|a| a.to_string() == *shown) else { continue };
        let (Some(long), Some(var)) = (arg.get_long(), arg.get_env()) else { continue };
        if !given(long) {
            error.insert(kind, ContextValue::String(var.to_string_lossy().into_owned()));
        }
    }
    error
}

/// clap's own conflict error when `m`, parsed by `cmd`, has both `a` and `b`, each named as it was
/// given: the flag, or its variable.
pub fn conflict(cmd: &mut Command, m: &ArgMatches, a: &str, b: &str) -> Result<(), clap::Error> {
    if m.value_source(a).is_none() || m.value_source(b).is_none() {
        return Ok(());
    }
    let shown = |id: &str| {
        let arg = cmd.get_arguments().find(|arg| arg.get_id() == id);
        match (m.value_source(id), arg.and_then(Arg::get_env)) {
            (Some(ValueSource::EnvVariable), Some(var)) => var.to_string_lossy().into_owned(),
            _ => arg.map_or_else(|| id.to_owned(), |arg| arg.to_string()),
        }
    };
    let (shown_a, shown_b) = (shown(a), shown(b));
    let mut error = clap::Error::new(ErrorKind::ArgumentConflict).with_cmd(cmd);
    error.insert(ContextKind::InvalidArg, ContextValue::String(shown_a));
    error.insert(ContextKind::PriorArg, ContextValue::String(shown_b));
    error.insert(ContextKind::Usage, ContextValue::StyledStr(cmd.render_usage()));
    Err(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_port_accepts_valid_numbers() {
        assert_eq!(control_port("50494"), Ok(ControlPort::Local(50494)));
        assert_eq!(control_port(" 8080 "), Ok(ControlPort::Local(8080)));
        assert_eq!(control_port("0"), Ok(ControlPort::Local(0)));
    }

    #[test]
    fn control_port_takes_the_port_of_a_loopback_host_and_port() {
        for loopback in ["127.0.0.1:7979", "localhost:7979", "[::1]:7979", " 127.0.0.1:7979 "] {
            assert_eq!(control_port(loopback), Ok(ControlPort::Local(7979)), "{loopback:?}");
        }
        for remote in ["10.0.0.5:7979", "example.com:7979", "[::2]:7979"] {
            assert_eq!(control_port(remote), Ok(ControlPort::Remote(remote.to_owned())), "{remote:?}");
        }
    }

    #[test]
    fn control_port_rejects_anything_else() {
        for bad in ["78787", "-1", "", "x", ":7979", "localhost:x", "::1:7979"] {
            assert_eq!(control_port(bad), Err(NotATarget), "{bad:?}");
        }
    }

    #[test]
    fn control_target_is_a_port_or_host_and_port() {
        for good in ["7979", "0", "localhost:7979", "127.0.0.1:1", "[::1]:7979"] {
            assert_eq!(control_target(good).map(|t| t.as_str().to_owned()), Ok(good.to_owned()), "{good:?}");
        }
        for spaced in [" 7979", "localhost:7979 "] {
            assert_eq!(control_target(spaced).map(|t| t.as_str().to_owned()), Ok(spaced.trim().to_owned()), "{spaced:?}");
        }
        for bad in ["", "x", "78787", ":7979", "host:", "host:x", "-1", "::1:7979", "[::1:7979"] {
            assert_eq!(control_target(bad), Err(NotATarget), "{bad:?}");
        }
    }

    /// A command with one port flag backed by a variable no test sets.
    fn port_command() -> Command {
        Command::new("t").arg(Arg::new("port").long("port").env("SOUNDCRAFT_TEST_UNSET_PORT").value_parser(control_port))
    }

    #[test]
    fn name_env_vars_names_the_variable_only_when_the_flag_was_not_given() {
        let mut cmd = port_command();
        let e = cmd.try_get_matches_from_mut(["t", "--port", "x"]).unwrap_err();
        let flag = name_env_vars(e, &cmd, &["--port".into(), "x".into()]);
        assert_eq!(flag.get(ContextKind::InvalidArg), Some(&ContextValue::String("--port <port>".into())));
        let e = cmd.try_get_matches_from_mut(["t", "--port=x"]).unwrap_err();
        // As the error from the variable would read: the flag is not among the arguments.
        let var = name_env_vars(e, &cmd, &[]);
        assert_eq!(var.get(ContextKind::InvalidArg), Some(&ContextValue::String("SOUNDCRAFT_TEST_UNSET_PORT".into())));
    }

    #[test]
    fn blank_env_is_unset_keeps_an_unset_variable() {
        let arg = blank_env_is_unset(Arg::new("port").env("SOUNDCRAFT_TEST_UNSET_PORT"));
        assert_eq!(arg.get_env(), Some(std::ffi::OsStr::new("SOUNDCRAFT_TEST_UNSET_PORT")));
    }

    #[test]
    fn conflict_is_a_clap_usage_error_naming_both_flags() {
        let mut cmd = port_command().arg(Arg::new("connect").long("connect"));
        let m = cmd.try_get_matches_from_mut(["t", "--port", "1", "--connect", "2"]).unwrap();
        let e = conflict(&mut cmd, &m, "port", "connect").unwrap_err();
        assert_eq!((e.kind(), e.exit_code()), (ErrorKind::ArgumentConflict, 2));
        assert!(e.to_string().starts_with("error: the argument '--port <port>' cannot be used with '--connect <connect>'\n\nUsage: t"), "{e}");
        let m = cmd.try_get_matches_from_mut(["t", "--port", "1"]).unwrap();
        assert!(conflict(&mut cmd, &m, "port", "connect").is_ok());
    }
}
