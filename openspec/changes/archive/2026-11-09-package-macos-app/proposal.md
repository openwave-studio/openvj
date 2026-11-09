# Proposal

## Why

openVJ runs as a bare `openvj` binary. macOS ties camera and microphone permission to whatever launched it (Terminal, iTerm, VS Code), and a binary without an Info.plist has no usage strings, so on the maintainer's Mac cameras failed with "camera access was denied". A real `openVJ.app` asks for camera and microphone access in its own name, can be started from Finder or the Dock, and is what v0.1 ships as.

## What Changes

- `scripts/bundle-macos.sh` builds a release binary and wraps it in `openVJ.app`: Info.plist with `NSCameraUsageDescription` and `NSMicrophoneUsageDescription`, Syphon.framework in `Contents/Frameworks`, and the build machine's absolute rpath removed.
- The app is signed ad hoc by default. When `OPENVJ_SIGN_IDENTITY` is set it signs with that identity and the hardened runtime, with entitlements for camera and audio input, ready for notarization later.
- The script zips the app as `openVJ-<version>-macos-arm64.zip`.
- A `macOS app` GitHub workflow runs the script on PRs that change packaging, on demand, and on `v*` tags. On a tag it attaches the zip to a draft GitHub release.
- README: how to build the app and how to open an ad hoc signed download (Gatekeeper's "Open Anyway").

Added after review: bundle ID `org.openvj.OpenVJ`, Developer ID signing and notarization for manual runs and tags once the maintainer's secrets are set, and an app icon from a PNG the maintainer provides.

FFmpeg (the maintainer, 2026-11-09: "bundle ffmpeg so users can play videos out of the box"): CI builds a decode-only, LGPL FFmpeg 7.1 (`scripts/build-ffmpeg-macos.sh`) with static libvpx for WebM alpha and FFmpeg's own HAP decoder, caches it, and the app ships `ffmpeg` and `ffprobe` in `Contents/MacOS`, signed like the app, with FFmpeg's and libvpx's licences and a source offer in `Contents/Resources/ThirdParty`. openVJ looks for them next to its own executable before PATH and Homebrew; `OPENVJ_FFMPEG_DIR` still overrides. Encoders, network protocols and ffplay are left out.

Out of scope: Intel or universal builds, opening `.avc` files by double-click, Linux and Windows packages.

## Capabilities

### New Capabilities
- `macos-app`: openVJ ships as a signed macOS app that asks for camera and microphone access in its own name.

### Modified Capabilities

## Impact

Packaging only; no app code changes. The `openvj` binary still runs from `cargo run` as before.

## Touches

- New: `scripts/bundle-macos.sh`, `scripts/build-ffmpeg-macos.sh`, `packaging/macos/Info.plist`, `packaging/macos/entitlements.plist`, `.github/workflows/macos-app.yml`
- `crates/openvj-media/src/lib.rs` (`tool_path`: look next to the executable)
- README.md (shared hot spot): "Running it" section
- Specs: new `macos-app`

## Conflicts

None known. The open PR #15 (Syphon flip) touches only `crates/openvj-media/src/live.rs`. The README edit is small; whichever merges second rebases.

## Open questions

Resolved by the maintainer: no personal name in the bundle ID; notarize releases with the maintainer's Apple Developer account; the maintainer makes the icon.
