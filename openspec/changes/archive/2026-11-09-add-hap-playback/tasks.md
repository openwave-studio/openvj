# Tasks

## 1. Decoder

- [x] 1.1 Share the DXV frame worker (clock following, look-ahead) as a generic player
- [x] 1.2 `hap` module: section headers, Snappy and chunked (decode instructions) frames, HAP Q Alpha's two images, format probe
- [x] 1.3 Tests: hand-built frames for every section type and corrupt input; FFmpeg-encoded HAP / HAP Alpha / HAP Q (snappy, none, chunks) cross-checked against FFmpeg's decoder

## 2. GPU

- [x] 2.1 `BcLayout` for straight BC3, BC7, scaled YCoCg and scaled YCoCg + BC4 alpha; shader modes
- [x] 2.2 Render test for each new layout

## 3. App

- [x] 3.1 Open HAP with the GPU player, falling back to FFmpeg; codec label
- [x] 3.2 README "What works"
- [ ] 3.3 The maintainer tests HAP clips from their own library in a downloaded build
