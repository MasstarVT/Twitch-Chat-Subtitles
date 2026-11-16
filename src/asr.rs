//! Streaming speech recognition with sherpa-onnx (runs fully offline).

use crate::args::Args;
use crate::server::Hub;
use sherpa_onnx::{OnlineRecognizer, OnlineRecognizerConfig, OnlineStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

pub struct Model {
    pub dir: PathBuf,
    tokens: PathBuf,
    encoder: PathBuf,
    decoder: PathBuf,
    joiner: PathBuf,
}

/// Finds a streaming transducer model (encoder/decoder/joiner + tokens.txt).
/// Looks in `--model` if given, otherwise in `models/` next to the exe and
/// in the current folder. Each of those may hold the model directly or in a
/// sub-folder (the way the downloaded archives unpack).
pub fn find_model(explicit: Option<&Path>) -> Result<Model, String> {
    let roots: Vec<PathBuf> = match explicit {
        Some(p) => vec![p.to_path_buf()],
        None => {
            let mut r = Vec::new();
            if let Some(dir) = std::env::current_exe()
                .ok()
                .and_then(|e| e.parent().map(Path::to_path_buf))
            {
                r.push(dir.join("models"));
            }
            r.push(PathBuf::from("models"));
            r
        }
    };

    for root in &roots {
        if let Some(m) = model_in(root) {
            return Ok(m);
        }
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        let mut subdirs: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect();
        subdirs.sort();
        if let Some(m) = subdirs.iter().find_map(|d| model_in(d)) {
            return Ok(m);
        }
    }

    Err(match explicit {
        Some(p) => format!(
            "No streaming model in \"{}\". The folder needs tokens.txt plus encoder, decoder and joiner .onnx files.",
            p.display()
        ),
        None => "No speech model found. Run scripts\\download-model.ps1 (or download-model.sh), \
                 or point to one with --model <folder>."
            .into(),
    })
}

fn model_in(dir: &Path) -> Option<Model> {
    let tokens = dir.join("tokens.txt");
    if !tokens.is_file() {
        return None;
    }
    Some(Model {
        encoder: pick_onnx(dir, "encoder")?,
        decoder: pick_onnx(dir, "decoder")?,
        joiner: pick_onnx(dir, "joiner")?,
        tokens,
        dir: dir.to_path_buf(),
    })
}

/// Picks `<prefix>*.onnx`, preferring the int8 build (smaller and faster) when both exist.
fn pick_onnx(dir: &Path, prefix: &str) -> Option<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            name.starts_with(prefix) && name.ends_with(".onnx")
        })
        .collect();
    files.sort();
    let int8 = files
        .iter()
        .position(|p| p.to_string_lossy().contains(".int8."));
    match int8 {
        Some(i) => Some(files.swap_remove(i)),
        None => files.into_iter().next(),
    }
}

pub fn create_recognizer(model: &Model, args: &Args) -> Result<OnlineRecognizer, String> {
    let path = |p: &Path| Some(p.to_string_lossy().into_owned());

    let mut cfg = OnlineRecognizerConfig::default();
    cfg.model_config.transducer.encoder = path(&model.encoder);
    cfg.model_config.transducer.decoder = path(&model.decoder);
    cfg.model_config.transducer.joiner = path(&model.joiner);
    cfg.model_config.tokens = path(&model.tokens);
    cfg.model_config.num_threads = args.threads;
    cfg.decoding_method = Some("greedy_search".into());
    cfg.enable_endpoint = true;
    // Silence needed to end a line before anything was recognised / after speech.
    cfg.rule1_min_trailing_silence = 2.4;
    cfg.rule2_min_trailing_silence = args.pause;
    cfg.rule3_min_utterance_length = args.max_sentence;

    OnlineRecognizer::create(&cfg).ok_or_else(|| {
        format!(
            "Could not load the model in \"{}\". Is it a streaming (online) transducer model?",
            model.dir.display()
        )
    })
}

/// Feeds audio to the recognizer and broadcasts text until the audio source closes.
pub fn run(
    recognizer: &OnlineRecognizer,
    audio: Receiver<Vec<f32>>,
    sample_rate: i32,
    hub: &Hub,
    verbose: bool,
) {
    let stream = recognizer.create_stream();
    let mut shown = String::new();

    for chunk in audio {
        stream.accept_waveform(sample_rate, &chunk);
        step(recognizer, &stream, &mut shown, hub, verbose, false);
    }

    // Audio source finished (WAV mode): flush whatever is left.
    stream.input_finished();
    step(recognizer, &stream, &mut shown, hub, verbose, true);
}

fn step(
    recognizer: &OnlineRecognizer,
    stream: &OnlineStream,
    shown: &mut String,
    hub: &Hub,
    verbose: bool,
    flush: bool,
) {
    while recognizer.is_ready(stream) {
        recognizer.decode(stream);
    }
    let text = recognizer
        .get_result(stream)
        .map(|r| r.text.trim().to_string())
        .unwrap_or_default();

    if text != *shown {
        if !text.is_empty() {
            hub.send(&event("partial", &text));
            if verbose {
                println!("  ~ {text}");
            }
        }
        shown.clone_from(&text);
    }

    if flush || recognizer.is_endpoint(stream) {
        if !text.is_empty() {
            hub.send(&event("final", &text));
            println!("> {text}");
        }
        recognizer.reset(stream);
        shown.clear();
    }
}

/// Builds the small JSON message sent to the overlay.
pub fn event(kind: &str, text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 32);
    out.push_str("{\"type\":\"");
    out.push_str(kind);
    out.push_str("\",\"text\":\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push_str("\"}");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_is_escaped_json() {
        assert_eq!(
            event("final", "Hi \"chat\"\\\n"),
            r#"{"type":"final","text":"Hi \"chat\"\\\n"}"#
        );
        assert_eq!(
            event("partial", "a\u{1}b"),
            r#"{"type":"partial","text":"a\u0001b"}"#
        );
    }

    #[test]
    fn prefers_int8_and_finds_nested_models() {
        let root = std::env::temp_dir().join(format!("tcs-model-test-{}", std::process::id()));
        let dir = root.join("some-model");
        std::fs::create_dir_all(&dir).unwrap();
        for f in [
            "tokens.txt",
            "encoder-epoch-99.onnx",
            "encoder-epoch-99.int8.onnx",
            "decoder-epoch-99.onnx",
            "joiner-epoch-99.onnx",
        ] {
            std::fs::write(dir.join(f), b"").unwrap();
        }

        let m = find_model(Some(&root)).expect("model found");
        assert_eq!(m.dir, dir);
        assert!(m.encoder.ends_with("encoder-epoch-99.int8.onnx"));
        assert!(m.decoder.ends_with("decoder-epoch-99.onnx"));

        std::fs::remove_dir_all(&root).unwrap();
        assert!(find_model(Some(&root)).is_err());
    }
}
