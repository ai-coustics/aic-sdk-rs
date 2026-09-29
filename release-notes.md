### Breaking Changes

- Removed unnecessary `Result` returns from methods whose native errors are prevented by the Rust bindings:
  - `ProcessorContext::parameter`, `VadContext::parameter`, and `EnergyVadContext::parameter` now return `f32` directly.
  - `ProcessorContext::reset`, `VadContext::reset`, `EnergyVadContext::reset`, and `Analyzer::reset` now return `()`.
  - `Processor::terminate_session`, `Vad::terminate_session`, and `Analyzer::terminate_session` now return `()`; the async methods on `ProcessorAsync` and `VadAsync` also return `()` when awaited.

### New Features

- Added `EnergyVadContext`, created through `Processor::energy_vad_context` or `ProcessorAsync::energy_vad_context`, for energy-based voice activity
detection using enhancement output. This is the same energy-based VAD available in pre-0.22 SDK versions. It behaves differently from the previous version.
It now picks up quiet and distant speech much more reliably, so you'll miss fewer words on speakerphones and in far-field setups.
It also triggers more often on background noise and background voices. If you see too many false activations, lower the sensitivity parameter (default 6.0).
A value around 4.0 gives a false-activation rate close to the previous version while still detecting more distant speech.

### Improvements

- Added new SIMD-enabled operations in AirTen, yielding better inference performance.

### Improvements

- SDK-internal error reports now include more detail on why a backend request failed.

### Bug Fixes

- Fixed `experimental.audio.output_clipping_samples` to count clipping in the final mixed output.

### Platform Support

- Added the Linux musl release targets `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`. The published `libaic.a` supports the default static linking only; the `dynamic-linking` and `runtime-linking` features have no musl `libaic.so` to bind against.
