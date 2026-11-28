mod markers;

use std::path::PathBuf;

use clap::Parser;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the time marker file of the audio,
    /// typically in Audacity label format (*.txt)
    /// but also supports .vtt and .srt (not recommended because of limited precision)
    input_time_markers: PathBuf,

    /// Path to the input audio file
    input_audio: PathBuf,

    /// Path to the target time marker file of the audio,
    /// typically in Audacity label format (*.txt)
    /// but also supports .vtt and .srt (not recommended because of limited precision)
    target_time_markers: PathBuf,

    /// Path to the target audio file to be generated
    /// if not provided, the program may generate a default output path based on the input files
    target_audio: Option<PathBuf>,
    // TODO: method options
}

fn main() {
    let mut cli = Args::parse();
    if cli.target_audio.is_none() {
        // make .warp.extended filename for the target audio if not provided
        let mut out = cli.input_audio.file_stem().unwrap().to_owned();
        out.push(".warped.");
        out.push(cli.input_audio.extension().unwrap());
        cli.target_audio = Some(cli.input_audio.with_file_name(out));
    }
    dbg!(cli);
}
