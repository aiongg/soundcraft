//! The CLI run in-process through `soundcraft_cli::run`: the tests that need no files, working
//! directory, environment or standard input. Those that do run the real binary (`behaviour.rs`).
//! These pin the hand-written parser's behaviour, quirks included; a change to it updates them on purpose.

use soundcraft_cli::run;

/// What [`run`] returned and wrote.
struct Invoke {
    code: i32,
    stdout: String,
    stderr: String,
}

fn invoke(args: &[&str]) -> Invoke {
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let code = run(args.iter().copied(), &mut stdout, &mut stderr);
    Invoke { code, stdout: String::from_utf8(stdout).unwrap(), stderr: String::from_utf8(stderr).unwrap() }
}

/// Exit 1, nothing on stdout, and exactly `error: {message}` on stderr.
#[track_caller]
fn assert_fails(got: &Invoke, message: &str) {
    assert_eq!(got.code, 1, "stderr: {}", got.stderr);
    assert_eq!(got.stdout, "");
    assert_eq!(got.stderr, format!("error: {message}\n"));
}

/// Exit 0 with nothing on either stream.
#[track_caller]
fn assert_silent_success(got: &Invoke) {
    assert_eq!((got.code, got.stdout.as_str(), got.stderr.as_str()), (0, "", ""));
}

const USAGE_FIRST_LINE: &str = "soundcraft-cli info FILE                       audio file or .scraft summary";
const USAGE_LAST_LINE: &str = "soundcraft-cli plugins                          list built-in plugins";
const NO_ARGS_USAGE: &str = "usage: soundcraft-cli <info|convert|run|script|commands|describe|app|mcp|parity|plugins> …  (see --help)\n";

/// The OS's message for a missing file: error 2 is `ENOENT` on Unix and `ERROR_FILE_NOT_FOUND` on Windows.
fn not_found() -> std::io::Error {
    std::io::Error::from_raw_os_error(2)
}

/// Exit 1 with the refused-connection message for `port`; the OS's text for the refusal differs by platform.
#[track_caller]
fn assert_refused(got: &Invoke, port: u16) {
    assert_eq!((got.code, got.stdout.as_str()), (1, ""));
    let prefix = format!("error: cannot reach SoundCraft at 127.0.0.1:{port}: ");
    assert!(got.stderr.starts_with(&prefix) && got.stderr.contains(" (os error ") && got.stderr.ends_with(")\n"), "{}", got.stderr);
}

#[test]
fn top_level_help_spellings_print_the_usage_block_to_stdout() {
    let long = invoke(&["--help"]);
    assert_eq!(long.code, 0);
    assert_eq!(long.stderr, "");
    assert!(long.stdout.starts_with(USAGE_FIRST_LINE), "{}", long.stdout);
    assert!(long.stdout.ends_with(&format!("{USAGE_LAST_LINE}\n")), "{}", long.stdout);
    for spelling in ["-h", "help"] {
        let got = invoke(&[spelling]);
        assert_eq!((got.code, got.stdout, got.stderr), (0, long.stdout.clone(), String::new()), "{spelling}");
    }
}

#[test]
fn top_level_version_spellings_print_the_version_to_stdout() {
    let expected = format!("SoundCraft CLI {}\n", env!("CARGO_PKG_VERSION"));
    for spelling in ["--version", "-V", "version"] {
        let got = invoke(&[spelling]);
        assert_eq!((got.code, got.stdout, got.stderr), (0, expected.clone(), String::new()), "{spelling}");
    }
}

#[test]
fn no_arguments_prints_a_one_line_usage_to_stderr_and_fails() {
    let got = invoke(&[]);
    assert_eq!((got.code, got.stdout.as_str(), got.stderr.as_str()), (1, "", NO_ARGS_USAGE));
}

#[test]
fn unknown_subcommand_fails_naming_it() {
    assert_fails(&invoke(&["bogus"]), "unknown subcommand `bogus`");
    // Only the first argument is the subcommand: `--help` after it does not rescue it.
    assert_fails(&invoke(&["bogus", "--help"]), "unknown subcommand `bogus`");
    // Subcommand names are case sensitive and not abbreviated.
    assert_fails(&invoke(&["INFO"]), "unknown subcommand `INFO`");
    assert_fails(&invoke(&["plug"]), "unknown subcommand `plug`");
    // A leading unknown flag is an unknown subcommand too.
    assert_fails(&invoke(&["--bogus"]), "unknown subcommand `--bogus`");
}

#[test]
fn info_and_script_read_help_as_a_file_name() {
    for sub in ["info", "script"] {
        for flag in ["--help", "-h"] {
            assert_fails(&invoke(&[sub, flag]), &format!("{flag}: {}", not_found()));
        }
    }
}

#[test]
fn convert_and_app_help_fall_through_to_their_usage_or_connection_error() {
    // convert needs two positionals; one flag is not enough.
    for flag in ["--help", "-h"] {
        assert_fails(&invoke(&["convert", flag]), "convert IN OUT");
    }
    // `app --help` has no method, so it prints its usage; `app -h` takes `-h` as the method and
    // tries the default port (7979) -- pinned via an explicit refused port.
    assert_fails(&invoke(&["app", "--help"]), "app [--port P] METHOD [JSON]");
    assert_refused(&invoke(&["app", "--port", "1", "-h"]), 1);
}

#[test]
fn run_rejects_long_help_but_ignores_short_help() {
    assert_fails(&invoke(&["run", "--help"]), "unknown option --help");
    // `-h` has no `--` prefix, so the option check skips it like any positional.
    assert_silent_success(&invoke(&["run", "-h"]));
}

#[test]
fn describe_reads_help_as_a_command_id() {
    assert_fails(&invoke(&["describe", "--help"]), "unknown command `--help`");
    assert_fails(&invoke(&["describe", "-h"]), "unknown command `-h`");
}

#[test]
fn commands_reads_help_as_a_filter_that_matches_nothing() {
    for flag in ["--help", "-h"] {
        assert_silent_success(&invoke(&["commands", flag]));
    }
}

#[test]
fn parity_and_plugins_ignore_help_and_print_their_normal_output() {
    let parity = invoke(&["parity"]);
    let plugins = invoke(&["plugins"]);
    for flag in ["--help", "-h"] {
        let got = invoke(&["parity", flag]);
        assert_eq!((got.code, got.stdout, got.stderr), (0, parity.stdout.clone(), String::new()), "parity {flag}");
        let got = invoke(&["plugins", flag]);
        assert_eq!((got.code, got.stdout, got.stderr), (0, plugins.stdout.clone(), String::new()), "plugins {flag}");
    }
}

#[test]
fn run_and_script_value_flags_without_a_value_fail() {
    for flag in ["--cmd", "--in", "--save", "--bounce", "--start", "--end"] {
        assert_fails(&invoke(&["run", flag]), &format!("{flag} needs a value"));
    }
    for flag in ["--in", "--save", "--bounce", "--start", "--end"] {
        assert_fails(&invoke(&["script", "s.txt", flag]), &format!("{flag} needs a value"));
    }
    // A following `--` word is the next flag, not the value.
    assert_fails(&invoke(&["run", "--bounce", "--inspect"]), "--bounce needs a value");
    assert_fails(&invoke(&["script", "s.txt", "--demo", "--bounce"]), "--bounce needs a value");
}

#[test]
fn run_boolean_flags_reject_an_inline_value() {
    assert_fails(&invoke(&["run", "--demo=1"]), "--demo takes no value");
    assert_fails(&invoke(&["run", "--inspect=1"]), "--inspect takes no value");
    assert_fails(&invoke(&["script", "s.txt", "--keep-going=1"]), "--keep-going takes no value");
}

#[test]
fn run_demo_and_sample_are_the_same_flag() {
    let demo = invoke(&["run", "--demo", "--inspect"]);
    let sample = invoke(&["run", "--sample", "--inspect"]);
    assert_eq!((demo.code, sample.code), (0, 0));
    assert_eq!(demo.stdout, sample.stdout);
    assert!(demo.stdout.contains("\"name\": \"Verb\""), "{}", demo.stdout);
}

#[test]
fn run_command_failures() {
    assert_fails(&invoke(&["run", "--cmd", "nosuch.cmd"]), "nosuch.cmd: unknown command `nosuch.cmd`");
    assert_fails(&invoke(&["run", "--cmd", "track.new={"]), "track.new: bad JSON: EOF while parsing an object at line 1 column 1");
    assert_fails(&invoke(&["run", "--in", "nope.scraft"]), &format!("nope.scraft: {}", not_found()));
    assert_fails(&invoke(&["run", "--in=nope.scraft"]), &format!("nope.scraft: {}", not_found()));
}

#[test]
fn commands_filters_by_id_or_label_and_uses_only_the_first_argument() {
    let all = invoke(&["commands"]);
    assert_eq!((all.code, all.stderr.as_str()), (0, ""));
    assert!(all.stdout.starts_with("session.new "), "{}", all.stdout);
    let bounce = invoke(&["commands", "bounce"]);
    assert!(bounce.stdout.contains("file.bounce_mix ") && !bounce.stdout.contains("session.new"), "{}", bounce.stdout);
    assert_eq!(invoke(&["commands", "bounce", "extra"]).stdout, bounce.stdout);
    // Matching is case-insensitive on the label only: an upper-case filter finds the labels, not the ids.
    assert!(invoke(&["commands", "BOUNCE"]).stdout.contains("Bounce Mix..."));
    assert_silent_success(&invoke(&["commands", "zzzzqq"]));
}

#[test]
fn describe_prints_json_for_a_known_id_and_fails_for_an_unknown_one() {
    let got = invoke(&["describe", "file.bounce_mix"]);
    assert_eq!((got.code, got.stderr.as_str()), (0, ""));
    assert!(got.stdout.starts_with("{\n  \"id\": \"file.bounce_mix\",\n  \"label\": \"Bounce Mix...\""), "{}", got.stdout);
    assert_fails(&invoke(&["describe", "nosuch"]), "unknown command `nosuch`");
}

#[test]
fn plugins_lists_one_line_per_plugin() {
    let got = invoke(&["plugins"]);
    assert_eq!((got.code, got.stderr.as_str()), (0, ""));
    assert!(got.stdout.starts_with("eq_1band             EQ 1-Band              Eq\n"), "{}", got.stdout);
}

#[test]
fn app_reports_a_refused_connection_and_checks_arguments_first() {
    for args in [
        vec!["app", "--port", "1", "session.inspect"],
        vec!["app", "--port=1", "session.inspect"],
        vec!["app", "session.inspect", "--port", "1"],
        vec!["app", "--port", "1", "session.inspect", "{}", "extra"],
    ] {
        assert_refused(&invoke(&args), 1);
    }
    assert_fails(&invoke(&["app", "--port", "1", "session.inspect", "{bad"]), "session.inspect: bad JSON: key must be a string at line 1 column 2");
    assert_fails(&invoke(&["app", "--port", "abc", "session.inspect"]), "127.0.0.1:abc: invalid port value");
}

#[test]
fn mcp_argument_errors_fail_before_reading_stdin() {
    assert_fails(&invoke(&["mcp", "--connect"]), "--connect needs a port or host:port");
    assert_fails(&invoke(&["mcp", "--automation-read-root"]), "--automation-read-root needs a folder");
    assert_fails(
        &invoke(&["mcp", "--automation-read-root", "--demo"]),
        "--automation-read-root needs a folder; use --automation-read-root=<dir> for a folder starting with '-'",
    );
    assert_fails(
        &invoke(&["mcp", "--automation-read-root", "missing-dir"]),
        &format!("cannot open the automation read root `missing-dir`: {}", not_found()),
    );
    assert_fails(
        &invoke(&["mcp", "--automation-read-root", ".", "--connect", "1"]),
        "--automation-read-root and --automation-write-root confine the in-process server; a running app is not confined by them, so drop --connect (or use --connect without the roots)",
    );
}
