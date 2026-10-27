/// Print to a writer without panicking on a closed pipe. A write error must not skip later work
/// such as `--bounce` or `--save`, and must not by itself report success.
macro_rules! outln {
    ($w:expr, $($t:tt)*) => {{
        let _ = writeln!($w, $($t)*);
    }};
}
