# Tasks

## 1. Engine

- [x] 1.1 Optional BGRA copy of the output in `openvj-render`, made only while asked for, with a test

## 2. Syphon server

- [x] 2.1 `syphon_out.rs`: create a Syphon Metal server on wgpu's Metal device, publish a wgpu texture on wgpu's Metal command queue
- [x] 2.2 Publish after each frame while a client is connected; recreate the server when the composition size changes
- [x] 2.3 Settings → Output switch, on by default; server listed by the app name alone

## 3. Docs and check

- [x] 3.1 README "What works"
- [x] 3.2 The maintainer checks a downloaded build in Synesthesia, MadMapper or OBS: right way up, right size, Activity Monitor unchanged with no client
