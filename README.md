# Twitch Chat Subtitles

Live subtitles of your microphone for OBS. Speech recognition runs **fully offline on your own PC**,
in a small Rust program that uses about **3% of one CPU core**.

- Words show up while you're still talking and are finalised when you pause.
- Each word animates in (rise, fade, pop, blur, drop or type-on), and older lines glide up smoothly.
- Punctuation and capital letters come from the default English model.
- The overlay is an OBS **Browser Source**, so it works with any scene and needs no plugin.
- The setup page has a live preview for font, size, colours, outline, background box, position, line count and auto-hide.
- One self-contained `.exe`, with no Python, no Visual C++ redistributable and no cloud account.

```
 Mic ──► capture (cpal/WASAPI) ──► speech recognizer (sherpa-onnx, streaming) ──► tiny web server ──► OBS Browser Source
                                                                                   │
                                                                                   └─► http://127.0.0.1:7878/  (setup page)
```

## Quick start (Windows)

1. **Download it.** Get the latest `.zip` from [Releases](https://github.com/MasstarVT/Twitch-Chat-Subtitles/releases/latest) and unzip it anywhere.
   To compile it yourself instead, see [Building from source](#building-from-source).

2. **Download the speech model.** Double-click `download-model.bat`. It saves about 70 MB to a `models` folder next to the exe.

3. **Run it.** Double-click `twitch-chat-subtitles.exe`.
   It uses your default microphone and prints what it hears.
   ```
   Twitch Chat Subtitles v0.1.0
     Model:    models\sherpa-onnx-streaming-zipformer-en-kroko-2025-08-06
     Mic:      Microphone (USB Mic) (48000 Hz, 2 ch)
     Setup:    http://127.0.0.1:7878/          <- open in a browser to style the subtitles
     Overlay:  http://127.0.0.1:7878/overlay   <- add to OBS as a Browser Source (1920x1080)
   ```

4. **Style it.** Open <http://127.0.0.1:7878/> in your browser, adjust the look, and click **Copy link**.

5. **Add it to OBS.** Choose *Sources → + → Browser*, paste the link as the URL, and set width `1920` and height `1080`.

To move it, copy the `.exe` and the `models` folder together. The app looks for `models\` next to the exe.

## Building from source

Install Rust from [rustup.rs](https://rustup.rs) (on Windows it needs the Visual Studio C++ build tools), then run:

```bash
cargo build --release
```

That produces `target\release\twitch-chat-subtitles.exe`. Run `download-model.bat` from the repo root to get the model.
The exe finds the repo's `models\` folder when it runs from `target\release`.

## Choosing a microphone

```bash
twitch-chat-subtitles.exe --list-devices
twitch-chat-subtitles.exe --device "USB Mic"
```

`--device` matches any part of the name. If your interface puts the mic on only one channel
(common with XLR interfaces), add `--channel 1` so the empty channel doesn't halve the volume.

To start it with your settings by double-clicking, make a `start.bat` next to the exe:

```bat
@echo off
twitch-chat-subtitles.exe --device "USB Mic" --pause 0.6
pause
```

## Options

| Option | Default | What it does |
|---|---|---|
| `--list-devices` | | List microphones and exit |
| `--device <NAME>` | system default | Microphone to use (any part of its name) |
| `--channel <N>` | mix all | Use only input channel N |
| `--model <DIR>` | first model in `models\` | Speech model folder |
| `--port <PORT>` | `7878` | Web server port |
| `--host <IP>` | `127.0.0.1` | Listen address. Use `0.0.0.0` for a two-PC setup, then point OBS at `http://<this-pc-ip>:7878/overlay` |
| `--pause <SEC>` | `0.8` | How long a pause ends a sentence |
| `--max-sentence <SEC>` | `15` | Force a line break after this long |
| `--threads <N>` | `1` | CPU threads for recognition |
| `--wav <FILE>` | | Transcribe a WAV file instead of the mic (testing) |
| `-v`, `--verbose` | | Also print in-progress text |

## Overlay URL parameters

The setup page writes these for you. You can also edit them in the OBS URL directly.
Colours are hex without `#` (`ffcc00`, or `000000a6` with transparency).

| Parameter | Default | Meaning |
|---|---|---|
| `size` | `44` | Font size in px |
| `font` | Segoe UI | Font family; add `gfont=1` to load it from Google Fonts |
| `weight` | `700` | 400 to 900 |
| `color` | `ffffff` | Text colour |
| `partial` | `ffffffcc` | Colour of words still being recognised |
| `stroke`, `strokecolor` | `0`, `000000` | Text outline width (px) and colour |
| `shadow` | on | `0` turns off the drop shadow |
| `bg` | `000000a6` | Box background (`00000000` for none) |
| `radius` | `14` | Box corner radius in px |
| `width` | `80` | Max box width, % of the screen |
| `fixed` | off | `1` keeps the box at full width |
| `lines` | `2` | Lines shown; older text scrolls off |
| `pos` | `bottom` | `bottom` or `top` |
| `align` | `center` | `left`, `center` or `right` |
| `margin` | `6` | Gap from the screen edge, % of height |
| `fade` | `6` | Hide after this many seconds of silence (`0` = never) |
| `case` | `none` | `sentence`, `upper` or `lower` |
| `anim` | `rise` | Word animation: `rise`, `fade`, `pop`, `blur`, `drop`, `type` or `none` |
| `demo` | off | `1` plays sample text, for positioning without talking |

## Why it's light

- One recognizer thread. The audio callback only converts samples and hands them over.
- The web server is about 160 lines of the Rust standard library, with no async runtime and no web framework.
  Updates are pushed to OBS with Server-Sent Events.
- sherpa-onnx and the ONNX runtime are statically linked. That's why the exe is about 19 MB, but it has no DLLs.
- Measured on Windows 11 with the default model: about 3% of one CPU core and about 175 MB RAM, mostly the model.

## Other languages and models

Any **streaming transducer** model from the
[sherpa-onnx model list](https://github.com/k2-fsa/sherpa-onnx/releases/tag/asr-models) works.
Pass its name to the download script, for example German:

```bash
powershell -ExecutionPolicy Bypass -File scripts\download-model.ps1 -Model sherpa-onnx-streaming-zipformer-de-kroko-2025-08-06
twitch-chat-subtitles.exe --model models\sherpa-onnx-streaming-zipformer-de-kroko-2025-08-06
```

French (`fr`) and Spanish (`es`) Kroko models have the same format.
Some older models output ALL CAPS; use **Letter case → Sentence case** on the setup page for those.

## Troubleshooting

- **Nothing shows in OBS.** Start the app before OBS, or right-click the Browser Source and choose *Refresh*. The overlay reconnects by itself if the app restarts.
- **No text appears in the console.** Check that the right mic is selected (`--list-devices`), and try `-v` to see in-progress text.
- **"Could not use port 7878".** Another copy is already running, or another app uses that port. Use `--port 7879`, and change the OBS URL to match.
- **Windows SmartScreen warning.** The exe isn't code-signed. Choose *More info → Run anyway*, or build it yourself.

## Building on Linux or macOS

Both `cpal` and `sherpa-onnx` support Linux and macOS, so `cargo build --release` and `scripts/download-model.sh` should work there.
This has only been tested on Windows so far.

## Credits and licenses

- This project: MIT, © 2026 MasstarVT. See [LICENSE](LICENSE).
- [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) (Apache-2.0) does the speech recognition.
- [cpal](https://github.com/RustAudio/cpal) (Apache-2.0) captures audio.
- The default model is [Kroko ASR](https://huggingface.co/Banafo/Kroko-ASR) by Banafo, a CC-BY-SA community model.
  It's downloaded separately and isn't part of this repository.
