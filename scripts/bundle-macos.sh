#!/usr/bin/env bash
# Builds openVJ.app and a zip of it in target/macos/.
#
#   scripts/bundle-macos.sh            # release build, signed ad hoc
#   OPENVJ_SIGN_IDENTITY="Developer ID Application" scripts/bundle-macos.sh
#
# With a real identity, setting OPENVJ_NOTARY_KEY (path to an App Store Connect API key, .p8),
# OPENVJ_NOTARY_KEY_ID and OPENVJ_NOTARY_ISSUER also notarizes the zip and staples the ticket,
# so a downloaded copy opens without Gatekeeper's "Open Anyway".
#
# packaging/macos/AppIcon.png (1024×1024), if present, becomes the app icon.
#
# FFmpeg: ffmpeg and ffprobe from target/ffmpeg/ (built by scripts/build-ffmpeg-macos.sh) go in
# Contents/MacOS, where openVJ looks first, with their licences in Contents/Resources/ThirdParty.
# Without them the app falls back to Homebrew's FFmpeg; OPENVJ_REQUIRE_FFMPEG=1 makes that an error.
#
# The app carries Info.plist usage strings, so macOS asks for camera and microphone
# access for openVJ itself rather than for the terminal it was started from.
# Extra arguments go to `cargo build` (for example `--no-default-features --features camera,syphon`).
set -euo pipefail

if [[ "$(uname -s)" != Darwin ]]; then
  echo "bundle-macos.sh only runs on macOS" >&2
  exit 1
fi

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
short_version="${version%%-*}" # 0.1.0-alpha.1 -> 0.1.0 (CFBundleShortVersionString takes numbers only)
arch="$(uname -m)"
identity="${OPENVJ_SIGN_IDENTITY:--}"

echo "==> Building openvj $version ($arch)"
cargo build --release -p openvj-app --bin openvj "$@"

out="target/macos"
app="$out/openVJ.app"
contents="$app/Contents"
bin="$contents/MacOS/openvj"
rm -rf "$app"
mkdir -p "$contents/MacOS" "$contents/Resources" "$contents/Frameworks"

cp target/release/openvj "$bin"
sed -e "s/__VERSION__/$version/" -e "s/__SHORT_VERSION__/$short_version/" \
  packaging/macos/Info.plist >"$contents/Info.plist"
printf 'APPL????' >"$contents/PkgInfo"

# Bundled ISF shaders (the Ghost Arcade pack, AGPL-3.0, with its LICENSE and NOTICE).
if [[ -d shaders ]]; then
  ditto shaders "$contents/Resources/shaders"
fi

if [[ -f packaging/macos/AppIcon.png ]]; then
  echo "==> Making the app icon"
  iconset="$out/AppIcon.iconset"
  rm -rf "$iconset"
  mkdir -p "$iconset"
  for size in 16 32 128 256 512; do
    sips -z "$size" "$size" packaging/macos/AppIcon.png --out "$iconset/icon_${size}x${size}.png" >/dev/null
    double=$((size * 2))
    sips -z "$double" "$double" packaging/macos/AppIcon.png --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
  done
  iconutil -c icns "$iconset" -o "$contents/Resources/AppIcon.icns"
  rm -rf "$iconset"
  /usr/libexec/PlistBuddy -c "Add :CFBundleIconFile string AppIcon" "$contents/Info.plist"
fi

# Absolute rpaths point into this machine's target/ folder (see openvj-app's build.rs).
# Syphon.framework sits in one of them: copy it into the app, then drop those rpaths so the
# app only loads from @executable_path/../Frameworks.
abs_rpaths=()
while IFS= read -r rpath; do
  abs_rpaths+=("$rpath")
done < <(otool -l "$bin" | awk '/cmd LC_RPATH/ {getline; getline; print $2}' | grep '^/' || true)

if otool -L "$bin" | grep -q '@rpath/Syphon.framework'; then
  syphon=""
  for rpath in ${abs_rpaths[@]+"${abs_rpaths[@]}"}; do
    [[ -d "$rpath/Syphon.framework" ]] && syphon="$rpath/Syphon.framework"
  done
  if [[ -z "$syphon" ]]; then
    echo "error: openvj links Syphon.framework but it wasn't found in its rpaths" >&2
    exit 1
  fi
  echo "==> Bundling $syphon"
  ditto "$syphon" "$contents/Frameworks/Syphon.framework"
fi
for rpath in ${abs_rpaths[@]+"${abs_rpaths[@]}"}; do
  install_name_tool -delete_rpath "$rpath" "$bin"
done

ffmpeg_dir="target/ffmpeg"
if [[ -x "$ffmpeg_dir/bin/ffmpeg" && -x "$ffmpeg_dir/bin/ffprobe" ]]; then
  echo "==> Bundling FFmpeg"
  cp "$ffmpeg_dir/bin/ffmpeg" "$ffmpeg_dir/bin/ffprobe" "$contents/MacOS/"
  mkdir -p "$contents/Resources/ThirdParty"
  ditto "$ffmpeg_dir/licenses" "$contents/Resources/ThirdParty/FFmpeg"
elif [[ "${OPENVJ_REQUIRE_FFMPEG:-}" == 1 ]]; then
  echo "error: no FFmpeg in $ffmpeg_dir; run scripts/build-ffmpeg-macos.sh first" >&2
  exit 1
else
  echo "warning: no FFmpeg in $ffmpeg_dir, so the app will need Homebrew's (scripts/build-ffmpeg-macos.sh builds one)" >&2
fi

echo "==> Signing (${identity/#-/ad hoc})"
sign=(codesign --force --sign "$identity")
if [[ "$identity" != - ]]; then
  # A real identity gets the hardened runtime and a secure timestamp, as notarization needs.
  sign+=(--options runtime --timestamp)
fi
# Nested code first, the app last: codesign seals what's inside when it signs the app.
for tool in ffmpeg ffprobe; do
  if [[ -f "$contents/MacOS/$tool" ]]; then
    "${sign[@]}" --identifier "org.openvj.OpenVJ.$tool" "$contents/MacOS/$tool"
  fi
done
if [[ -d "$contents/Frameworks/Syphon.framework" ]]; then
  "${sign[@]}" "$contents/Frameworks/Syphon.framework"
fi
if [[ "$identity" != - ]]; then
  sign+=(--entitlements packaging/macos/entitlements.plist)
fi
"${sign[@]}" "$app"

echo "==> Checking the bundle"
plutil -lint "$contents/Info.plist"
codesign --verify --deep --strict --verbose=2 "$app"
rpaths="$(otool -l "$bin" | awk '/cmd LC_RPATH/ {getline; getline; print $2}')"
if grep -q '^/' <<<"$rpaths"; then
  echo "error: absolute rpath left in the app" >&2
  exit 1
fi
if [[ -d "$contents/Frameworks/Syphon.framework" ]] && ! grep -qx '@executable_path/../Frameworks' <<<"$rpaths"; then
  echo "error: openvj has no rpath to Contents/Frameworks" >&2
  exit 1
fi
for key in NSCameraUsageDescription NSMicrophoneUsageDescription; do
  /usr/libexec/PlistBuddy -c "Print :$key" "$contents/Info.plist" >/dev/null
done

zip="$out/openVJ-$version-macos-$arch.zip"
rm -f "$zip"
ditto -c -k --keepParent "$app" "$zip"

if [[ "$identity" != - && -n "${OPENVJ_NOTARY_KEY:-}" ]]; then
  echo "==> Notarizing (this takes a few minutes)"
  xcrun notarytool submit "$zip" --wait \
    --key "$OPENVJ_NOTARY_KEY" --key-id "$OPENVJ_NOTARY_KEY_ID" --issuer "$OPENVJ_NOTARY_ISSUER"
  xcrun stapler staple "$app"
  # Zip again so the download carries the stapled ticket and opens offline too.
  rm -f "$zip"
  ditto -c -k --keepParent "$app" "$zip"
  spctl --assess --type execute --verbose "$app"
fi
echo "==> $app"
echo "==> $zip"
