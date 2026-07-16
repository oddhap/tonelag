#!/usr/bin/env bash
set -euo pipefail

FFMPEG_VERSION=8.1.2
FFMPEG_SHA256=464beb5e7bf0c311e68b45ae2f04e9cc2af88851abb4082231742a74d97b524c
OPENMPT_VERSION=0.8.7
OPENMPT_SHA256=275c29ef47be9992f62a35fcc96f7ca05c06d2fd05c9298b8dee9f743f75b089

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
WORK=${TONELAG_BUILD_DIR:-"$ROOT/.build/audio-libs"}
PREFIX=${TONELAG_PREFIX:-"$WORK/prefix"}
SOURCES="$WORK/sources"
JOBS=${JOBS:-4}

mkdir -p "$SOURCES" "$PREFIX"

fetch() {
  local url=$1 destination=$2 expected=$3
  if [[ ! -f "$destination" ]]; then
    curl --fail --location --retry 3 "$url" --output "$destination"
  fi
  printf '%s  %s\n' "$expected" "$destination" | shasum -a 256 --check
}

OPENMPT_ARCHIVE="$SOURCES/libopenmpt-${OPENMPT_VERSION}.tar.gz"
FFMPEG_ARCHIVE="$SOURCES/ffmpeg-${FFMPEG_VERSION}.tar.xz"
fetch "https://lib.openmpt.org/files/libopenmpt/src/libopenmpt-${OPENMPT_VERSION}+release.autotools.tar.gz" "$OPENMPT_ARCHIVE" "$OPENMPT_SHA256"
fetch "https://ffmpeg.org/releases/ffmpeg-${FFMPEG_VERSION}.tar.xz" "$FFMPEG_ARCHIVE" "$FFMPEG_SHA256"

if [[ ${TONELAG_FETCH_ONLY:-0} == 1 ]]; then
  printf 'Verified FFmpeg %s and libopenmpt %s source archives in %s\n' \
    "$FFMPEG_VERSION" "$OPENMPT_VERSION" "$SOURCES"
  exit 0
fi

rm -rf "$WORK/libopenmpt" "$WORK/ffmpeg"
mkdir -p "$WORK/libopenmpt" "$WORK/ffmpeg"
tar -xzf "$OPENMPT_ARCHIVE" --strip-components=1 -C "$WORK/libopenmpt"
tar -xJf "$FFMPEG_ARCHIVE" --strip-components=1 -C "$WORK/ffmpeg"

(
  cd "$WORK/libopenmpt"
  ./configure --prefix="$PREFIX" --disable-static --enable-shared --disable-openmpt123
  make -j"$JOBS"
  make install
)

export PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
(
  cd "$WORK/ffmpeg"
  ./configure \
    --prefix="$PREFIX" \
    --disable-everything \
    --enable-shared --disable-static --enable-pic \
    --disable-programs --disable-doc --disable-debug --disable-network --disable-autodetect \
    --disable-avdevice --disable-avfilter --disable-swscale --disable-postproc \
    --enable-avcodec --enable-avformat --enable-avutil --enable-swresample \
    --enable-libopenmpt \
    --enable-protocol=file,pipe \
    --enable-demuxer=aac,aiff,ape,asf,flac,libopenmpt,mov,mp3,musepack,ogg,wav,wv \
    --enable-parser=aac,aac_latm,flac,mpegaudio,opus,vorbis \
    --enable-decoder=aac,aac_fixed,alac,ape,flac,mp1,mp1float,mp2,mp2float,mp3,mp3float,musepack7,musepack8,opus,vorbis,wavpack,wmav1,wmav2,wmalossless,wmapro,pcm_s16be,pcm_s16le,pcm_s24be,pcm_s24le,pcm_s32be,pcm_s32le,pcm_f32be,pcm_f32le,pcm_f64be,pcm_f64le,pcm_u8 \
    --extra-cflags="-I$PREFIX/include" \
    --extra-ldflags="-L$PREFIX/lib"
  cp ffbuild/config.log "$PREFIX/ffmpeg-config.log"
  make -j"$JOBS"
  make install
  ./ffbuild/config.mak > "$PREFIX/ffmpeg-config.txt"
)

mkdir -p "$PREFIX/share/tonelag/sources"
cp "$OPENMPT_ARCHIVE" "$FFMPEG_ARCHIVE" "$PREFIX/share/tonelag/sources/"
printf 'Built FFmpeg %s and libopenmpt %s in %s\n' "$FFMPEG_VERSION" "$OPENMPT_VERSION" "$PREFIX"
