# media-sources Specification

## Purpose

Clips play images, video files and built-in GPU generators.

## Requirements

### Requirement: Images
Clips SHALL play png, jpg, gif, webp and bmp images.

#### Scenario: Image clip
- **WHEN** a png is placed and launched
- **THEN** it shows on its layer for its duration

### Requirement: Video files
Clips SHALL play common video files (mov, mp4, mkv, webm, avi and others) decoded by FFmpeg, found on PATH, in /opt/homebrew/bin or in OPENVJ_FFMPEG_DIR. Video SHALL seek and play backwards for Bounce and Random.

#### Scenario: No FFmpeg
- **GIVEN** FFmpeg is not installed
- **WHEN** a non-DXV video is launched
- **THEN** loading that clip fails with an "is FFmpeg installed?" error, and images, generators and DXV still work

### Requirement: Built-in generators
openVJ SHALL ship GPU generators (Plasma, Tunnel, Bars, Rings, Noise, Kaleido, Gradient, Strobe, Checker), each with parameters and a speed, with Bars, Rings and Strobe locked to the BPM. New generators are planned for v0.4.

#### Scenario: Tempo-locked generator
- **WHEN** Strobe plays at 120 BPM
- **THEN** it flashes in time with the beat
