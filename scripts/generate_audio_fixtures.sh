#!/usr/bin/env bash
set -euo pipefail

output=${1:-tests/fixtures/audio}
mkdir -p "$output"
base="$output/source.wav"
ffmpeg -hide_banner -loglevel error -y -f lavfi -i "sine=frequency=1000:sample_rate=48000:duration=2" -c:a pcm_s16le "$base"

ffmpeg -hide_banner -loglevel error -y -i "$base" -c:a libmp3lame "$output/tone.mp3"
ffmpeg -hide_banner -loglevel error -y -i "$base" -c:a mp2 "$output/tone.mp2"
ffmpeg -hide_banner -loglevel error -y -i "$base" -c:a aac "$output/tone.m4a"
ffmpeg -hide_banner -loglevel error -y -i "$base" -c:a aac -f adts "$output/tone.aac"
ffmpeg -hide_banner -loglevel error -y -i "$base" -c:a flac "$output/tone.flac"
ffmpeg -hide_banner -loglevel error -y -i "$base" -c:a alac "$output/tone-alac.m4a"
ffmpeg -hide_banner -loglevel error -y -i "$base" -ac 2 -c:a vorbis -strict experimental "$output/tone.ogg"
ffmpeg -hide_banner -loglevel error -y -i "$base" -c:a libopus "$output/tone.opus"
ffmpeg -hide_banner -loglevel error -y -i "$base" -c:a wavpack "$output/tone.wv"
ffmpeg -hide_banner -loglevel error -y -i "$base" -c:a wmav2 "$output/tone.wma"
ffmpeg -hide_banner -loglevel error -y -i "$base" "$output/tone.aiff"
cp "$base" "$output/corrupt.wav"
truncate -s 128 "$output/corrupt.wav"
