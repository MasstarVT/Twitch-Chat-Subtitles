//! Twitch Chat Subtitles: live, offline speech-to-text from your mic,
//! shown as subtitles in an OBS Browser Source.

mod args;
mod asr;
mod audio;
mod server;

use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let double_clicked = raw.is_empty();

    if let Err(e) = run(raw) {
        eprintln!("\nError: {e}");
        if double_clicked {
            // Keep the window open so the message can be read.
            eprintln!("\nPress Enter to close.");
            let _ = std::io::stdin().read_line(&mut String::new());
        }
        std::process::exit(1);
    }
}

fn run(raw: Vec<String>) -> Result<(), String> {
    let args = match args::parse(raw)? {
        args::Parsed::Run(a) => a,
        args::Parsed::Exit => return Ok(()),
    };
    if args.list_devices {
        return audio::list_devices();
    }

    println!("Twitch Chat Subtitles v{}", env!("CARGO_PKG_VERSION"));

    let model = asr::find_model(args.model.as_deref())?;
    println!("  Model:    {}", model.dir.display());
    let recognizer = asr::create_recognizer(&model, &args)?;

    let hub = Arc::new(server::Hub::default());
    let url = server::start(&args.host, args.port, hub.clone())?;

    // Audio flows from the mic (or a WAV file) to the recognizer through this channel.
    let (tx, rx) = mpsc::sync_channel::<Vec<f32>>(256);

    let (_capture, sample_rate) = match &args.wav {
        Some(path) => {
            let path = path.to_string_lossy().into_owned();
            let wave = sherpa_onnx::Wave::read(&path).ok_or_else(|| {
                format!("Could not read \"{path}\" (it must be a 16-bit PCM WAV).")
            })?;
            println!("  Input:    {path} ({} Hz)", wave.sample_rate());
            let rate = wave.sample_rate();
            let samples = wave.samples().to_vec();
            thread::spawn(move || play_wav(&samples, rate, tx));
            (None, rate)
        }
        None => {
            let cap = audio::start(args.device.as_deref(), args.channel, tx)?;
            println!(
                "  Mic:      {} ({} Hz, {} ch)",
                cap.name, cap.sample_rate, cap.channels
            );
            let rate = cap.sample_rate as i32;
            (Some(cap), rate)
        }
    };

    println!("  Setup:    {url}          <- open in a browser to style the subtitles");
    println!("  Overlay:  {url}overlay   <- add to OBS as a Browser Source (1920x1080)");
    println!("\nListening. Ctrl+C to quit.\n");

    asr::run(&recognizer, rx, sample_rate, &hub, args.verbose);

    // Only reached when a WAV file has been fully played.
    thread::sleep(Duration::from_millis(500));
    println!("\nDone.");
    Ok(())
}

/// Streams a WAV file at real-time speed, so it behaves like a live mic.
fn play_wav(samples: &[f32], rate: i32, tx: mpsc::SyncSender<Vec<f32>>) {
    let chunk = (rate as usize / 10).max(1); // 100 ms
    let silence = vec![0.0; chunk];
    // Trailing silence lets the recognizer notice the end of the last sentence.
    let tail = std::iter::repeat_n(&silence[..], 15);
    for piece in samples.chunks(chunk).chain(tail) {
        if tx.send(piece.to_vec()).is_err() {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
}
