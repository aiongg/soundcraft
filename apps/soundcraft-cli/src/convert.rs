use super::args::{Options, arg_value, check_options};
use super::error::{CliError, ConvertError, UsageError};
use std::io::Write;

pub(crate) fn convert(args: &[String], out: &mut dyn Write) -> Result<(), CliError> {
    let (Some(input), Some(output)) = (args.first(), args.get(1)) else { return Err(UsageError::Convert.into()) };
    const OPTIONS: &Options = &[("--rate", true), ("--bit-depth", true), ("--normalize", false), ("--dither", false), ("--no-dither", false)];
    check_options(&args[2..], OPTIONS)?;
    let rate = parse_rate(arg_value(args, "--rate").as_deref())?;
    let bit_depth = parse_bit_depth(arg_value(args, "--bit-depth").as_deref())?;
    let dither_flag = parse_dither(args)?;
    let bytes = std::fs::read(input).map_err(|source| CliError::File { path: input.clone(), source })?;
    let ext = std::path::Path::new(input).extension().and_then(std::ffi::OsStr::to_str);
    let (info, mut buf) = soundcraft_audio_io::decode(&bytes, ext).map_err(|source| CliError::Audio { path: input.clone(), source })?;
    if let Some(rate) = rate {
        buf.channels = soundcraft_dsp::offline::resample(&buf.channels, buf.sample_rate, rate);
        buf.sample_rate = rate;
    }
    let normalize = args.iter().any(|a| a == "--normalize");
    if normalize {
        soundcraft_dsp::offline::normalize(&mut buf.channels, -0.1, false);
    }
    let processed = rate.is_some_and(|r| r != info.sample_rate) || normalize;
    let dither = dither_flag.unwrap_or_else(|| auto_dither(info.sample_format, bit_depth, processed));
    let out_ext = std::path::Path::new(output).extension().and_then(std::ffi::OsStr::to_str).unwrap_or("wav").to_ascii_lowercase();
    let Some(format) = soundcraft_audio_io::encode_format_for(&out_ext) else {
        return Err(ConvertError::OutputFormat { output: output.clone(), ext: out_ext }.into());
    };
    let opts = soundcraft_audio_io::EncodeOptions { format, bit_depth, dither, bwf: None };
    let bytes = soundcraft_audio_io::encode(&buf, &opts).map_err(|source| CliError::Audio { path: output.clone(), source })?;
    std::fs::write(output, bytes).map_err(|source| CliError::File { path: output.clone(), source })?;
    outln!(out, "wrote {output} ({} ch, {} Hz, {:.2} s)", buf.num_channels(), buf.sample_rate, buf.duration_secs());
    Ok(())
}

fn parse_rate(v: Option<&str>) -> Result<Option<u32>, ConvertError> {
    match v {
        None => Ok(None),
        Some(r) => match r.parse::<u32>() {
            Ok(hz) if hz > 0 => Ok(Some(hz)),
            _ => Err(ConvertError::Rate(r.to_owned())),
        },
    }
}

fn parse_bit_depth(v: Option<&str>) -> Result<soundcraft_audio_io::BitDepth, ConvertError> {
    match v {
        None | Some("24") => Ok(soundcraft_audio_io::BitDepth::Int24),
        Some("16") => Ok(soundcraft_audio_io::BitDepth::Int16),
        Some("32") | Some("32f") => Ok(soundcraft_audio_io::BitDepth::Float32),
        Some(b) => Err(ConvertError::BitDepth(b.to_owned())),
    }
}

/// `--dither` / `--no-dither`: `Some` when the caller chose, `None` to decide from the depths.
fn parse_dither(args: &[String]) -> Result<Option<bool>, ConvertError> {
    match (args.iter().any(|a| a == "--dither"), args.iter().any(|a| a == "--no-dither")) {
        (true, true) => Err(ConvertError::DitherConflict),
        (true, false) => Ok(Some(true)),
        (false, true) => Ok(Some(false)),
        (false, false) => Ok(None),
    }
}

/// Whether `convert` dithers when not told: only when writing integer samples that cannot hold the
/// source exactly, i.e. the output has fewer bits than the source, the source is float or lossy, or
/// resampling/normalizing produced new sample values. A 16-bit file written as 16 or 24-bit, or
/// anything written as 32-bit float, is copied sample for sample (digital silence stays silent).
fn auto_dither(source: soundcraft_audio_io::SampleFormat, target: soundcraft_audio_io::BitDepth, processed: bool) -> bool {
    use soundcraft_audio_io::{BitDepth, SampleFormat};
    let target_bits = match target {
        BitDepth::Float32 => return false,
        BitDepth::Int16 => 16,
        BitDepth::Int24 => 24,
        BitDepth::Int32 => 32,
    };
    if processed {
        return true;
    }
    match source {
        SampleFormat::Int8 => target_bits < 8,
        SampleFormat::Int16 => target_bits < 16,
        SampleFormat::Int24 => target_bits < 24,
        // Decoded through f32, which holds 24 bits: a 32-bit integer source has already lost
        // resolution, so treat it like float.
        SampleFormat::Int32 | SampleFormat::Float32 | SampleFormat::Float64 | SampleFormat::Compressed => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_rejects_invalid_rate_and_bit_depth() {
        assert_eq!(parse_rate(None), Ok(None));
        assert_eq!(parse_rate(Some("44100")), Ok(Some(44_100)));
        assert!(parse_rate(Some("4410x")).is_err());
        assert!(parse_rate(Some("0")).is_err());
        assert!(matches!(parse_bit_depth(None), Ok(soundcraft_audio_io::BitDepth::Int24)));
        assert!(matches!(parse_bit_depth(Some("16")), Ok(soundcraft_audio_io::BitDepth::Int16)));
        assert!(matches!(parse_bit_depth(Some("32f")), Ok(soundcraft_audio_io::BitDepth::Float32)));
        assert!(parse_bit_depth(Some("8")).is_err());
    }

    #[test]
    fn convert_dither_flags() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(parse_dither(&args(&["in.wav", "out.wav"])), Ok(None));
        assert_eq!(parse_dither(&args(&["in.wav", "out.wav", "--dither"])), Ok(Some(true)));
        assert_eq!(parse_dither(&args(&["in.wav", "out.wav", "--no-dither"])), Ok(Some(false)));
        assert!(parse_dither(&args(&["in.wav", "out.wav", "--dither", "--no-dither"])).is_err());
    }

    #[test]
    fn convert_dithers_only_when_resolution_is_lost() {
        use soundcraft_audio_io::{BitDepth, SampleFormat};
        // Same depth or wider: the samples fit exactly.
        assert!(!auto_dither(SampleFormat::Int16, BitDepth::Int16, false));
        assert!(!auto_dither(SampleFormat::Int16, BitDepth::Int24, false));
        assert!(!auto_dither(SampleFormat::Int24, BitDepth::Int24, false));
        assert!(!auto_dither(SampleFormat::Int8, BitDepth::Int16, false));
        // Float output never dithers.
        assert!(!auto_dither(SampleFormat::Float32, BitDepth::Float32, true));
        assert!(!auto_dither(SampleFormat::Int24, BitDepth::Float32, false));
        // Fewer bits, a float or lossy source, or processed samples: dither.
        assert!(auto_dither(SampleFormat::Int24, BitDepth::Int16, false));
        assert!(auto_dither(SampleFormat::Float32, BitDepth::Int24, false));
        assert!(auto_dither(SampleFormat::Compressed, BitDepth::Int16, false));
        assert!(auto_dither(SampleFormat::Int32, BitDepth::Int24, false));
        assert!(auto_dither(SampleFormat::Int16, BitDepth::Int16, true));
    }
}
