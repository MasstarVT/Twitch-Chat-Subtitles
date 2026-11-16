#!/usr/bin/env sh
# Downloads a streaming speech model into the "models" folder.
#
#   ./scripts/download-model.sh
#   ./scripts/download-model.sh sherpa-onnx-streaming-zipformer-de-kroko-2025-08-06
#
# Any streaming transducer model from
# https://github.com/k2-fsa/sherpa-onnx/releases/tag/asr-models works.
set -eu

model="${1:-sherpa-onnx-streaming-zipformer-en-kroko-2025-08-06}"
dest="$(cd "$(dirname "$0")/.." && pwd)/models"
url="https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/$model.tar.bz2"

if [ -f "$dest/$model/tokens.txt" ]; then
  echo "Already downloaded: $dest/$model"
  exit 0
fi

mkdir -p "$dest"
echo "Downloading $model ..."
curl -L --fail --progress-bar -o "$dest/$model.tar.bz2" "$url"
echo "Unpacking ..."
tar -xjf "$dest/$model.tar.bz2" -C "$dest"
rm "$dest/$model.tar.bz2"
echo "Done: $dest/$model"
