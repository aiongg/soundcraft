//! The desktop app's command line: `soundcraft [--demo] [--control <port>] [--no-audio] [session.scraft | audio files…]`.

/// What the command line asks for.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct LaunchArgs {
    /// `--version` or `-V`: print the version and exit.
    pub(crate) version: bool,
    /// `--demo` or `--sample`: start with the demo session.
    pub(crate) demo: bool,
    /// `--no-audio`: don't open an audio device.
    pub(crate) no_audio: bool,
    /// `--control <port>`, else `SOUNDCRAFT_CONTROL_PORT`.
    pub(crate) control: Option<u16>,
    /// Sessions, audio and MIDI files to open.
    pub(crate) files: Vec<String>,
}

/// Parses the arguments (without the program name). `control_env` is the value of
/// `SOUNDCRAFT_CONTROL_PORT`, used when `--control` is missing or not a port. A word that starts
/// with `--` and isn't a known flag is ignored.
pub(crate) fn parse(args: &[String], control_env: Option<&str>) -> LaunchArgs {
    let control = args
        .iter()
        .position(|a| a == "--control")
        .and_then(|i| args.get(i + 1))
        .and_then(|p| p.parse().ok())
        .or_else(|| control_env.and_then(|p| p.parse().ok()));
    let files = args
        .iter()
        .enumerate()
        .filter(|(i, a)| !(a.starts_with("--") || (i > &0 && args.get(i - 1).is_some_and(|p| p == "--control"))))
        .map(|(_, a)| a.clone())
        .collect();
    LaunchArgs {
        version: args.iter().any(|a| a == "--version" || a == "-V"),
        demo: args.iter().any(|a| a == "--demo" || a == "--sample"),
        no_audio: args.iter().any(|a| a == "--no-audio"),
        control,
        files,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str], env: Option<&str>) -> LaunchArgs {
        parse(&args.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>(), env)
    }

    #[test]
    fn flags_and_files() {
        let a = run(&["--demo", "--no-audio", "--control", "50494", "a.scraft", "b.wav"], None);
        assert_eq!(
            a,
            LaunchArgs { demo: true, no_audio: true, control: Some(50494), files: vec!["a.scraft".into(), "b.wav".into()], ..Default::default() }
        );
        assert!(run(&["--sample"], None).demo);
        assert!(run(&["-V"], None).version);
        assert!(run(&["--version"], None).version);
        assert_eq!(run(&[], None), LaunchArgs::default());
    }

    #[test]
    fn unknown_double_dash_flags_are_ignored_and_single_dash_words_are_files() {
        let a = run(&["--bogus", "-x", "a.wav"], None);
        assert_eq!(a.files, ["-x", "a.wav"]);
    }

    #[test]
    fn a_bad_control_port_falls_back_to_the_variable() {
        assert_eq!(run(&["--control", "nope"], Some("7")).control, Some(7));
        assert_eq!(run(&["--control", "70000"], None).control, None);
        assert_eq!(run(&["--control"], Some("x")).control, None);
        assert_eq!(run(&["--control", "5"], Some("7")).control, Some(5));
        assert_eq!(run(&["a.wav"], Some("7")).control, Some(7));
    }

    #[test]
    fn the_control_value_is_not_a_file() {
        assert_eq!(run(&["--control", "nope", "a.wav"], None).files, ["a.wav"]);
    }
}
