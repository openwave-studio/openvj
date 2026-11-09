# dxv-playback Specification

## Purpose

Resolume's DXV codec plays straight on the GPU, without FFmpeg, so many layers of HD DXV play without CPU-side pixel decoding.

## Requirements

### Requirement: Decode DXV on the GPU
DXV movies (normal and high quality, with or without alpha) SHALL be read from the QuickTime file and uploaded as compressed BC textures, decoding as current FFmpeg (7.0+) does.

#### Scenario: DXV clip
- **WHEN** a DXV .mov from Resolume Alley is launched
- **THEN** it plays without starting an FFmpeg process

### Requirement: Fallback
If the GPU lacks BC texture support or a DXV frame fails to decode, the clip SHALL fall back to FFmpeg.

#### Scenario: No BC support
- **GIVEN** a GPU without BC compression
- **WHEN** a DXV clip is launched
- **THEN** it plays through FFmpeg instead
