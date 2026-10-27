use super::error::{CliError, UsageError};
use soundcraft_automation::{Headless, Remote, mcp::Server};
use soundcraft_engine::Engine;
use soundcraft_engine::file_access::{self, AutomationRoots};
use std::io::Write;

/// What `mcp` was asked for.
struct McpArgs {
    connect: Option<String>,
    demo: bool,
    roots: Option<std::sync::Arc<AutomationRoots>>,
}

/// Parse the `mcp` options up front: a malformed or misspelled root flag must not leave the
/// server running unconfined.
fn parse_mcp_args(args: &[String]) -> Result<McpArgs, CliError> {
    let mut connect = None;
    let mut demo = false;
    let mut roots_args = crate::automation_args::AutomationArgs::default();
    let mut it = args.iter().cloned();
    while let Some(a) = it.next() {
        if roots_args.consume(&a, &mut it)? {
            continue;
        }
        match a.as_str() {
            "--connect" => connect = Some(it.next().ok_or(UsageError::ConnectNeedsPort)?),
            "--demo" => demo = true,
            other if other.starts_with("--automation-") => return Err(UsageError::UnknownMcpOption(other.to_owned()).into()),
            _ => {}
        }
    }
    let read: Vec<&std::path::Path> = roots_args.read_roots.iter().map(std::path::PathBuf::as_path).collect();
    let write: Vec<&std::path::Path> = roots_args.write_roots.iter().map(std::path::PathBuf::as_path).collect();
    let roots = AutomationRoots::new(&read, &write).map_err(CliError::Roots)?;
    if roots.is_some() && connect.is_some() {
        return Err(UsageError::RootsWithConnect.into());
    }
    Ok(McpArgs { connect, demo, roots })
}

/// Serves on the process's real stdin and stdout; only the notes go to `err`.
pub(crate) fn mcp(args: &[String], err: &mut dyn Write) -> Result<(), CliError> {
    let McpArgs { connect, demo, roots } = parse_mcp_args(args)?;
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let served = if let Some(port) = connect {
        let mut s = Server::new(Remote::new(&port));
        s.serve(stdin.lock(), stdout.lock())
    } else {
        // `--demo` is the operator's own choice, made before the roots come into force.
        let e = if demo { soundcraft_engine::demo::demo_engine() } else { Engine::default() };
        let mut h = Headless::new(e);
        if let Some(roots) = roots {
            // Every thread of this process, not only the server's: nothing reaches past the roots.
            file_access::confine_process(roots.clone());
            outln!(err, "soundcraft-cli: files confined: {}", roots.describe());
            h = h.with_automation_roots(Some(roots));
        }
        let mut s = Server::new(h);
        s.serve(stdin.lock(), stdout.lock())
    };
    served.map_err(CliError::Serve)
}
