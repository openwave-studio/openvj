#!/usr/bin/env bash
# Builds the FFmpeg that openVJ.app ships: ffmpeg and ffprobe, decode-only, LGPL, linked only
# against macOS system libraries. Output goes to target/ffmpeg/ (bin/, licenses/), which
# scripts/bundle-macos.sh copies into the app. Takes ~10 minutes; CI caches the result.
#
# What openVJ needs from FFmpeg (crates/openvj-media/src/lib.rs): probe and decode any video
# file, libvpx's VP8/VP9 decoders for transparent WebM, the HAP decoder (FFmpeg's built-in
# Snappy, no libsnappy), the scale filter, and raw RGBA out through a pipe.
set -euo pipefail

FFMPEG_VERSION=7.1.1
LIBVPX_VERSION=1.15.0

if [[ "$(uname -s)" != Darwin ]]; then
  echo "build-ffmpeg-macos.sh only runs on macOS" >&2
  exit 1
fi

root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/target/ffmpeg"
work="$root/target/ffmpeg-build"
prefix="$work/prefix"
jobs="$(sysctl -n hw.ncpu)"
export MACOSX_DEPLOYMENT_TARGET=11.0

case "$(uname -m)" in
  arm64) vpx_target=arm64-darwin20-gcc ;;
  *) echo "build-ffmpeg-macos.sh builds for Apple Silicon (arm64) only" >&2; exit 1 ;;
esac
command -v pkg-config >/dev/null || brew install pkgconf

rm -rf "$out" "$work"
mkdir -p "$work" "$out/bin" "$out/licenses"

echo "==> libvpx $LIBVPX_VERSION"
git clone -q --depth 1 --branch "v$LIBVPX_VERSION" https://chromium.googlesource.com/webm/libvpx "$work/libvpx"
(
  cd "$work/libvpx"
  ./configure --target="$vpx_target" --prefix="$prefix" --enable-static --disable-shared --enable-pic \
    --disable-examples --disable-tools --disable-docs --disable-unit-tests \
    --enable-vp8-decoder --enable-vp9-decoder --disable-vp8-encoder --disable-vp9-encoder \
    --enable-vp9-highbitdepth
  make -j"$jobs"
  make install
)

echo "==> FFmpeg $FFMPEG_VERSION"
# GitHub mirror of git.ffmpeg.org (faster in CI); same tags.
git clone -q --depth 1 --branch "n$FFMPEG_VERSION" https://github.com/FFmpeg/FFmpeg.git "$work/ffmpeg"
# No --enable-gpl and no --enable-nonfree: the result is LGPL-2.1-or-later.
configure_flags=(
  --prefix="$prefix"
  --enable-static --disable-shared
  --disable-autodetect --enable-videotoolbox --enable-zlib --enable-libvpx
  --disable-ffplay --disable-doc --disable-network --disable-debug
  --disable-encoders --enable-encoder=rawvideo,wrapped_avframe
  --disable-muxers --enable-muxer=rawvideo,null
  --pkg-config-flags=--static
  --extra-cflags="-mmacosx-version-min=$MACOSX_DEPLOYMENT_TARGET"
  --extra-ldflags="-mmacosx-version-min=$MACOSX_DEPLOYMENT_TARGET"
)
(
  cd "$work/ffmpeg"
  PKG_CONFIG_PATH="$prefix/lib/pkgconfig" ./configure "${configure_flags[@]}"
  make -j"$jobs"
  make install
)

cp "$prefix/bin/ffmpeg" "$prefix/bin/ffprobe" "$out/bin/"

echo "==> Checking"
for tool in ffmpeg ffprobe; do
  # Only macOS's own libraries and frameworks may be linked, or the app breaks on other Macs.
  if otool -L "$out/bin/$tool" | tail -n +2 | awk '{print $1}' | grep -Ev '^(/usr/lib/|/System/Library/)'; then
    echo "error: $tool links a non-system library (above)" >&2
    exit 1
  fi
done
decoders="$("$out/bin/ffmpeg" -hide_banner -decoders)"
for d in libvpx-vp9 libvpx hap h264 hevc prores dxv; do
  grep -qE "^ [A-Z.]{6} $d " <<<"$decoders" || { echo "error: decoder $d missing" >&2; exit 1; }
done
if grep -q -- '--enable-gpl' <<<"$("$out/bin/ffmpeg" -hide_banner -version)"; then
  echo "error: GPL build" >&2
  exit 1
fi

echo "==> Licences and source offer"
cp "$work/ffmpeg/COPYING.LGPLv2.1" "$out/licenses/FFmpeg-COPYING.LGPLv2.1"
cp "$work/ffmpeg/LICENSE.md" "$out/licenses/FFmpeg-LICENSE.md"
cp "$work/libvpx/LICENSE" "$out/licenses/libvpx-LICENSE"
cp "$work/libvpx/PATENTS" "$out/licenses/libvpx-PATENTS"
cat >"$out/licenses/README.txt" <<NOTICE
openVJ includes the ffmpeg and ffprobe programs from FFmpeg $FFMPEG_VERSION (https://ffmpeg.org),
licensed under the GNU Lesser General Public License version 2.1 or later
(FFmpeg-COPYING.LGPLv2.1), statically linked with libvpx $LIBVPX_VERSION (BSD licence,
libvpx-LICENSE and libvpx-PATENTS). FFmpeg is a trademark of Fabrice Bellard.

Source code:
  FFmpeg $FFMPEG_VERSION: https://ffmpeg.org/releases/ffmpeg-$FFMPEG_VERSION.tar.xz
                  (git: https://git.ffmpeg.org/ffmpeg.git, tag n$FFMPEG_VERSION), unmodified
  libvpx $LIBVPX_VERSION: https://chromium.googlesource.com/webm/libvpx, tag v$LIBVPX_VERSION, unmodified
  How they were built: scripts/build-ffmpeg-macos.sh in https://github.com/openwave-studio/openvj
For three years from the date you received this copy of openVJ, the openVJ maintainers will
also provide this source on request: open an issue at https://github.com/openwave-studio/openvj.

FFmpeg configure flags:
  ${configure_flags[*]}
NOTICE
sed -i '' "s|$prefix|<prefix>|g" "$out/licenses/README.txt"

rm -rf "$work"
echo "==> $out"
