//! Command-line flags, parsed by hand to keep the binary free of extra crates.

use std::path::PathBuf;

pub const HELP: &str = "\
Twitch Chat Subtitles - live mic subtitles for OBS, fully offline.

USAGE:
    twitch-chat-subtitles [OPTIONS]

OPTIONS:
    --list-devices         List microphones and exit
    --device <NAME>        Microphone to use (any part of its name). Default: system default mic
    --channel <N>          Use only input channel N (1, 2, ...) instead of mixing all channels
    --model <DIR>          Speech model folder. Default: first model found in ./models
    --port <PORT>          Web server port [default: 7878]
    --host <IP>            Address to listen on [default: 127.0.0.1]
                           Use 0.0.0.0 to reach the overlay from another PC (e.g. a 2-PC setup)
    --pause <SECONDS>      Silence that ends a sentence [default: 0.8]
    --max-sentence <SEC>   Force a line break after this long [default: 15]
    --threads <N>          CPU threads for speech recognition [default: 1]
    --wav <FILE>           Transcribe a 16-bit WAV file instead of the mic (for testing)
    -v, --verbose          Also print partial (in-progress) text
    -h, --help             Show this help
    -V, --version          Show the version

Once running, open http://127.0.0.1:7878/ in a browser to style the subtitles,
then add http://127.0.0.1:7878/overlay to OBS as a Browser Source.
";

pub struct Args {
    pub list_devices: bool,
    pub device: Option<String>,
    pub channel: Option<usize>,
    pub model: Option<PathBuf>,
    pub port: u16,
    pub host: String,
    pub pause: f32,
    pub max_sentence: f32,
    pub threads: i32,
    pub wav: Option<PathBuf>,
    pub verbose: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            list_devices: false,
            device: None,
            channel: None,
            model: None,
            port: 7878,
            host: "127.0.0.1".into(),
            pause: 0.8,
            max_sentence: 15.0,
            threads: 1,
            wav: None,
            verbose: false,
        }
    }
}

pub enum Parsed {
    Run(Args),
    Exit,
}

pub fn parse(raw: impl IntoIterator<Item = String>) -> Result<Parsed, String> {
    let mut a = Args::default();
    let mut it = raw.into_iter();

    while let Some(arg) = it.next() {
        // Accept both `--port 9000` and `--port=9000`.
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f.to_string(), Some(v.to_string())),
            _ => (arg, None),
        };
        let mut value = || {
            inline
                .clone()
                .or_else(|| it.next())
                .ok_or_else(|| format!("{flag} needs a value"))
        };

        match flag.as_str() {
            "-h" | "--help" => {
                print!("{HELP}");
                return Ok(Parsed::Exit);
            }
            "-V" | "--version" => {
                println!("twitch-chat-subtitles {}", env!("CARGO_PKG_VERSION"));
                return Ok(Parsed::Exit);
            }
            "--list-devices" => a.list_devices = true,
            "--device" => a.device = Some(value()?),
            "--channel" => {
                let n: usize = number(&flag, &value()?)?;
                if n == 0 {
                    return Err("--channel starts at 1".into());
                }
                a.channel = Some(n);
            }
            "--model" => a.model = Some(PathBuf::from(value()?)),
            "--port" => a.port = number(&flag, &value()?)?,
            "--host" => a.host = value()?,
            "--pause" => a.pause = number(&flag, &value()?)?,
            "--max-sentence" => a.max_sentence = number(&flag, &value()?)?,
            "--threads" => a.threads = number::<i32>(&flag, &value()?)?.max(1),
            "--wav" => a.wav = Some(PathBuf::from(value()?)),
            "-v" | "--verbose" => a.verbose = true,
            other => {
                return Err(format!(
                    "Unknown option {other}. Run with --help to see options."
                ));
            }
        }
    }
    Ok(Parsed::Run(a))
}

fn number<T: std::str::FromStr>(flag: &str, v: &str) -> Result<T, String> {
    v.trim()
        .parse()
        .map_err(|_| format!("{flag} expects a number, got \"{v}\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> Args {
        match parse(args.iter().map(|s| s.to_string())) {
            Ok(Parsed::Run(a)) => a,
            _ => panic!("expected Run"),
        }
    }

    #[test]
    fn defaults() {
        let a = run(&[]);
        assert_eq!(a.port, 7878);
        assert_eq!(a.host, "127.0.0.1");
        assert!(a.device.is_none());
    }

    #[test]
    fn both_value_styles() {
        let a = run(&["--port", "9000", "--device=Yeti", "--channel", "2", "-v"]);
        assert_eq!(a.port, 9000);
        assert_eq!(a.device.as_deref(), Some("Yeti"));
        assert_eq!(a.channel, Some(2));
        assert!(a.verbose);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse(["--port".to_string()]).is_err());
        assert!(parse(["--port".to_string(), "abc".to_string()]).is_err());
        assert!(parse(["--channel".to_string(), "0".to_string()]).is_err());
        assert!(parse(["--nope".to_string()]).is_err());
    }
}
