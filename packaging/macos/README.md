# macOS packaging

`scripts/bundle-macos.sh` builds `target/macos/openVJ.app` from `Info.plist` here, and the
[macOS app workflow](../../.github/workflows/macos-app.yml) runs it on GitHub.

- **Bundle ID** `org.openvj.OpenVJ`. macOS keeps camera and microphone permissions under it, so
  changing it means granting them again.
- **FFmpeg**: `scripts/build-ffmpeg-macos.sh` builds a decode-only, LGPL FFmpeg (with libvpx for
  transparent WebM, and FFmpeg's own HAP decoder) into `target/ffmpeg/`. The bundle script puts
  `ffmpeg` and `ffprobe` in `Contents/MacOS`, signs them like the app, and copies the licences and
  source offer to `Contents/Resources/ThirdParty/FFmpeg`. The workflow caches the build, keyed on
  the script, so changing the script (for example the FFmpeg version) rebuilds it.
- **Icon**: `AppIcon.svg` is the logo (the UI accent #4fe6da on #151329, the same background, canvas and corner radius as openDJ's logo, which uses its lilac the same way); `AppIcon.png` is it rendered at 1024×1024 on Apple's macOS icon grid (artwork 824×824, centred, transparent margin). The bundle script turns the PNG into `AppIcon.icns`. After changing the SVG, render the PNG again the same way (any SVG renderer; the margin keeps it the size of other Dock icons).
- **Signing**: ad hoc unless Developer ID secrets are set (below). Ad hoc builds need
  System Settings → Privacy & Security → **Open Anyway** the first time.

## Developer ID signing and notarization

Manual runs and `v*` tags sign with Developer ID and notarize once these repository secrets exist
(Settings → Secrets and variables → Actions → New repository secret). Pull requests stay ad hoc.

| Secret | What it is |
| --- | --- |
| `MACOS_CERT_P12` | The *Developer ID Application* certificate and its private key, exported as .p12, base64-encoded |
| `MACOS_CERT_PASSWORD` | The password you gave the .p12 export |
| `APPLE_API_KEY_P8` | Contents of an App Store Connect API key file (`AuthKey_XXXX.p8`) |
| `APPLE_API_KEY_ID` | That key's Key ID |
| `APPLE_API_ISSUER_ID` | The Issuer ID shown above the keys list |

1. **Certificate.** In Xcode → Settings → Accounts → your team → Manage Certificates, click + and
   choose *Developer ID Application* (only the account holder can create one). Then in Keychain
   Access → My Certificates, right-click "Developer ID Application: …" → Export as .p12 with a password.
   Run `base64 -i Certificates.p12 | pbcopy` and paste it as `MACOS_CERT_P12`. Delete the .p12 afterwards.
2. **API key.** In App Store Connect → Users and Access → Integrations → App Store Connect API →
   Team Keys, generate a key with *Developer* access. Download the .p8 (only possible once) and paste
   its whole text, including the BEGIN/END lines, as `APPLE_API_KEY_P8`. Copy the Key ID and Issuer ID too.
3. Run the workflow (Actions → macOS app → Run workflow). The log says "Notarizing", and the
   downloaded app opens without a Gatekeeper warning.

Locally the same works with `OPENVJ_SIGN_IDENTITY="Developer ID Application"` and
`OPENVJ_NOTARY_KEY` (path to the .p8), `OPENVJ_NOTARY_KEY_ID`, `OPENVJ_NOTARY_ISSUER`.
