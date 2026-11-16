# Downloads a streaming speech model into the "models" folder.
#
#   powershell -ExecutionPolicy Bypass -File scripts\download-model.ps1
#   powershell -ExecutionPolicy Bypass -File scripts\download-model.ps1 -Model sherpa-onnx-streaming-zipformer-de-kroko-2025-08-06
#
# Any streaming transducer model from
# https://github.com/k2-fsa/sherpa-onnx/releases/tag/asr-models works.

param(
    [string]$Model = "sherpa-onnx-streaming-zipformer-en-kroko-2025-08-06",
    [string]$Dest = (Join-Path (Split-Path -Parent $PSScriptRoot) "models")
)

$ErrorActionPreference = "Stop"
$url = "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/$Model.tar.bz2"
$target = Join-Path $Dest $Model

if (Test-Path (Join-Path $target "tokens.txt")) {
    Write-Host "Already downloaded: $target"
    exit 0
}

New-Item -ItemType Directory -Force -Path $Dest | Out-Null
$archive = Join-Path $Dest "$Model.tar.bz2"

Write-Host "Downloading $Model ..."
curl.exe -L --fail --progress-bar -o $archive $url
if ($LASTEXITCODE -ne 0) { throw "Download failed. Check the model name and your internet connection." }

Write-Host "Unpacking ..."
tar.exe -xjf $archive -C $Dest
if ($LASTEXITCODE -ne 0) { throw "Could not unpack $archive" }
Remove-Item $archive

Write-Host "Done: $target"
