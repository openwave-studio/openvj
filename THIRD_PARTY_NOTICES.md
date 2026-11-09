# Third-party notices

openVJ is licensed under the GPL-3.0-or-later (see [LICENSE](LICENSE)). It includes or ships
with the following third-party software and assets, each under its own licence. The macOS app
carries this file and the licence texts below in `openVJ.app/Contents/Resources/ThirdParty/`.

## Syphon.framework (macOS app)

Syphon, © 2010 bangnoise (Tom Butterworth) & vade (Anton Marini). BSD licence, reproduced in
full in [packaging/licenses/Syphon-LICENSE.txt](packaging/licenses/Syphon-LICENSE.txt) and in
Settings → About in the app. Bundled in `Contents/Frameworks` through the
[syphon-core](https://github.com/BlueJayLouche/syphon-rs) crate (MIT).
<https://syphon.github.io>

## FFmpeg and libvpx (macOS app)

The app includes `ffmpeg` and `ffprobe` from [FFmpeg](https://ffmpeg.org) 7.1.1, built by
[scripts/build-ffmpeg-macos.sh](scripts/build-ffmpeg-macos.sh) as a decode-only build under the
GNU Lesser General Public License version 2.1 or later (no GPL or non-free parts), statically
linked with [libvpx](https://chromium.googlesource.com/webm/libvpx) 1.15.0 (BSD licence and patent
grant). openVJ runs them as separate programs. The licence texts, the exact configure flags and
the source offer are in `Contents/Resources/ThirdParty/FFmpeg/`. FFmpeg is a trademark of Fabrice
Bellard.

## Ghost Arcade ISF shaders

The 364 shaders in [shaders/ghost-arcade](shaders/ghost-arcade) come from
[Ghost Arcade](https://github.com/riskcapital/ghost-arcade), © Risk Capital Media LLC and the
Ghost Arcade contributors, under the GNU Affero General Public License v3.0. Their licence and a
NOTICE crediting the author of every file are in that folder, and in
`Contents/Resources/shaders/ghost-arcade/` in the app. openVJ loads them at runtime as data files
and doesn't change them. Shaders derived from Shadertoy or other non-commercial sources are left
out.

## Fonts

- **Inter Tabular**: [Inter](https://github.com/rsms/inter) by Rasmus Andersson, with tabular
  digits made the default, under the SIL Open Font License 1.1
  ([OFL.txt](crates/openvj-app/ui/fonts/OFL.txt)).
- **Zen Dots** by Yoshimichi Ohira ([googlefonts/zen-dots](https://github.com/googlefonts/zen-dots)),
  unchanged, under the SIL Open Font License 1.1
  ([OFL-ZenDots.txt](crates/openvj-app/ui/fonts/OFL-ZenDots.txt)).

## Libraries

- [Slint](https://slint.dev) is used under its GPL-3.0 licence.
- [Ableton Link](https://github.com/Ableton/link), through the
  [rusty_link](https://crates.io/crates/rusty_link) crate, under the GPL-2.0-or-later.
- [wgpu](https://wgpu.rs), naga, cpal, rustfft, nokhwa, midir, roxmltree, image and the other Rust
  crates listed in [Cargo.lock](Cargo.lock) are compiled into openVJ under their own licences
  (mostly MIT and/or Apache-2.0). Each crate's licence is in its source package on
  [crates.io](https://crates.io).

## Acknowledgements

- [Varda](https://github.com/im-knots/varda) (MIT) is the reference for openVJ's projection
  mapping, its camera input design and the ISF binding style it accepts.
- [Resolume](https://resolume.com) is the model for openVJ's workflow and file formats. openVJ is
  not affiliated with or endorsed by Resolume.
