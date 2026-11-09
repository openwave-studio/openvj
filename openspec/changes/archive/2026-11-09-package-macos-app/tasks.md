# Tasks

## 1. Bundle

- [x] 1.1 Info.plist with camera and microphone usage strings, and entitlements for the hardened runtime
- [x] 1.2 `scripts/bundle-macos.sh`: release build, Syphon.framework in Contents/Frameworks, clean rpaths, ad hoc or identity signing, zip
- [x] 1.3 Verify the bundle in the script (plist lint, codesign verify, framework present)

## 2. CI

- [x] 2.1 `macos-app` workflow: on packaging PRs, on demand, and on `v*` tags with a draft release

## 3. Docs

- [x] 3.1 README: build the app, open an ad hoc signed download
- [x] 3.2 The maintainer tests a downloaded build: camera prompt, microphone prompt, Syphon (2026-11-09: cameras, audio input, video and Syphon to OBS work; a virtual camera still fails, tracked separately)

## 4. Release signing (the maintainer has an Apple Developer account)

- [x] 4.1 Bundle ID `org.openvj.OpenVJ`, without a personal name
- [x] 4.2 Developer ID signing and notarization on manual runs and tags, from repository secrets
- [x] 4.3 App icon from `packaging/macos/AppIcon.png` when present
- [x] 4.4 App icon from the maintainer's logo (packaging/macos/AppIcon.svg)
- [x] 4.5 Signing secrets: moved to the v0.1 release prep (the maintainer adds them to the org)

## 5. FFmpeg in the app

- [x] 5.1 `scripts/build-ffmpeg-macos.sh`: decode-only LGPL FFmpeg 7.1 with static libvpx, licences and source offer
- [x] 5.2 Workflow builds it once and caches it; the bundle script copies and signs `ffmpeg`/`ffprobe`
- [x] 5.3 `tool_path` looks next to the executable first
- [x] 5.4 CI smoke test: the bundled ffmpeg decodes, has `libvpx-vp9` and `hap`, and links only system libraries
- [x] 5.5 README: no Homebrew FFmpeg needed for the app
