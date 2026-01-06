# Changelog

## 0.13.0 - Unreleased

This release integrates ai-coustics C library version 0.13.0, which comes with a number of new features and several breaking changes.

Most notably, the C library does no longer include any models, which significantly reduces the library's binary size. The models are now available
separately for download at https://artifacts.ai-coustics.io.

### New features

- Added optional `download-model` feature and `Model::download` helper to fetch models from the ai-coustics artifact repository with manifest lookup, checksum verification, and compatibility checks via the new `get_compatible_model_version` API.
- Added `Model::from_buffer` and an `include_model!` macro that embeds model binaries with guaranteed 64-byte alignment, making it easier to ship models without separate files.
- Added a `Config` helper returned by `Processor::optimal_config` to pre-fill optimal sample rate and frame count before initialization.

### Breaking changes

- Separated model loading from audio processing: `Model` now only loads `.aicmodel` assets from disk or memory, while processing moved to a new `Processor` created from a borrowed `Model`.
- Removed `ModelType` and the `Model::new`/`initialize`/`process_*` processing APIs.
- Use `Model::from_file`/`from_buffer` to load models and `Processor::*` (`initialize`, `process_*`, `output_delay`, VAD creation) for processing.
- `Processor` is now lifetime-bound to the `Model` it was created from, preventing it from outliving the model.
- `Processor::initialize` now takes a `Config` struct (returned by `optimal_config`) instead of discrete arguments.
- `optimal_sample_rate` and `optimal_num_frames` now live on `Processor` and return plain values instead of `Result`.
- `EnhancementParameter` was renamed to `Parameter`, and `set_parameter`/`parameter` moved to `Processor`.
- `set_parameter` and `reset` now take `&self` instead of `&mut self`.
- `output_delay` now returns `usize` directly instead of `Result`.
- `AicError` gained a `ModelDownload` variant; exhaustive matches must handle it.
- VAD construction moved from `Model::create_vad` to `Processor::create_vad` and now only needs `&self`.

### Fixes

- Corrected VAD creation and docs for the new processor API, preventing misuse when reusing a model across processors.
- `process_planar` now rejects channel counts above the 16-channel maximum instead of panicking when `num_channels` exceeded the supported limit.

## 0.12.0 - 2025-12-12

### New features

- Added new VAD parameter `VadParameter::MinimumSpeechDuration` used to control for how long speech needs to be present
in the audio signal before the VAD considers it speech.

### Breaking changes

- Replaced VAD parameter `VadParameter::LookbackBufferSize` with `VadParameter::SpeechHoldDuration`, used to control
for how long the VAD continues to detect speech after the audio signal no longer contains speech.

## 0.11.0 - 2025-12-11

### New features

- Added new Quail Voice Focus STT model (`ModelType::QuailVfSttL16`), purpose-built to isolate and elevate the foreground speaker while suppressing both interfering speech and background noise.
- Added new variants of the Quail STT model: `ModelType::QuailSttL8`, `ModelType::QuailSttS16`, and `ModelType::QuailSttS8`.
- Added `Model::process_sequential` for sequential channel data in a single buffer.

### Breaking changes

- The `num_channels` and `num_frames` arguments have been removed from `Model::process_interleaved`'s function signature. These arguments are now inferred from the buffer size and the `num_channels` value passed to `Model::initialize`.
- `ModelType::QuailSTT` was renamed to `ModelType::QuailSttL16`.
- `ModelType::QuailXS` was renamed to `ModelType::QuailXs`.
- `ModelType::QuailXXS` was renamed to `ModelType::QuailXxs`.
- `Processor::create_vad` now takes `&self` instead of `&mut self`.

### Fixes

- VAD now works correctly when `EnhancementParameter::EnhancementLevel` is set to 0 or `EnhancementParameter::Bypass` is enabled (previously non-functional in these cases).

## 0.10.1 - 2025-12-03

### Breaking changes

- Rust version limitation changed from stable 1.91.1 to beta 1.92

### Fixes

- Fixed build errors when building crates with a dependency on the `ring` crate.

## 0.10.0 - 2025-11-17

### Features

- **Quail STT** (`ModelType::QuailSTT`): Our newest speech enhancement model is optimized for human-to-machine interaction (e.g., voice agents, speech-to-text). This model operates at a native sample rate of 16 kHz and uses fixed enhancement parameters that cannot be changed during runtime. The model is also compatible with our VAD.

- Derived `Hash` on `EnhancementParameter`, `VadParameter` and `ModelType`.

### Breaking Changes

- `Parameter` was renamed to `EnhancementParameter`.
- Renamed `Model::get_parameter` to `Model::parameter` and `Vad::get_parameter` to `Vad::parameter` to follow Rust standards.
- Removed Parameter **NoiseGateEnable** as it is now a fixed part of our VAD.
- Added new error code **ParameterFixed** returned when attempting to modify a parameter of a model with fixed parameters.

## Fixes

- Fixed an issue where `aic_vad_is_speech_detected` always returned `true` when `AIC_VAD_PARAMETER_LOOKBACK_BUFFER_SIZE` was set to `1.0`.

## 0.9.1 - 2025-11-17

## Features

- **Internal library patching**: Static libraries are now patched internally to simplify usage from Rust, reducing integration complexity
- **Windows support**: Added `x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc` as supported target platforms

## 0.9.0 - 2025-11-06

### Features
- **Voice Activity Detection**: This release adds a new Quail-based VAD. The VAD automatically uses the output of a Quail model to calculate a voice activity prediction.

### Breaking Changes
- `handle_error()`'s visibility is now `pub(crate)`.

## 0.8.2 - 2025-11-06

### Fixes
- Fixed build error on macOS

## 0.8.1 - 2025-10-29

### Fixes
- Fixed documentation build on docs.rs

## 0.8.0 - 2025-10-28

### New features
- **Self-Service Licenses**: Starting with this release, you can use self-service licenses directly from our development portal.
- **Usage-Based Telemetry**: This release introduces a new telemetry feature that collects usage data, paving the way for future usage-based pricing models such as pay-per-minute billing.
    - **What we collect**: We collect only the processing time used and some diagnostic data
    - **Privacy**: We do not collect any information about your audio content. Your audio never leaves your device during our processing.
    - **Requirements**: Requires a constant internet connection. If the SDK cannot be activated online, enhancement will stop after 10 seconds. If telemetry data cannot be sent, enhancement will stop after 5 minutes. When enhancement is stopped an error will be returned, the audio will be bypassed and the processing delay will be still applied to ensure an uninterrupted audio stream without discontinuities.
    - **Error Handling**: When processing is bypassed because our backend cannot be reached or does not allow you to process, the process functions will return `AicError::EnhancementNotAllowed`. Make sure to handle this error in your implementation.
    - **Offline Licenses**: If you cannot provide a constant internet connection, please contact us to obtain a special offline license that does not require telemetry.


### Breaking changes
- **Variable number of frames supported**: `Model::initialize` now supports a variable number of frames per call. To enable this feature, use the new `allow_variable_frames` parameter in the initialize function. Set allow_variable_frames to true to enable variable frame processing, or false to maintain the previous fixed frame behavior. Note that enabling variable frames results in higher processing delay.
- **New bypass parameter**: A new parameter `Parameter::Bypass` has been added to control audio processing bypass while preserving algorithmic delay. When enabled, the input audio passes through unmodified, but the output is still delayed by the same amount as during normal processing. This ensures seamless transitions when toggling enhancement on/off without audible clicks or timing shifts.
- **Updated Error Codes**: Expanded and renamed error variants, with additional license-related errors.
- **Version API improved**: `aic_sdk_version()` was renamed to `aic_sdk::get_version()` and it now returns a `&'static str`.

### Fixes
- The internal model state is now automatically reset when processing is paused (e.g., when bypass is enabled or enhancement level is set to 0). This ensures a clean state when processing resumes.
- The reset operation now ensures that all internal DSP components are properly reset, providing a more thorough clean state.
- Fixed an issue where, after a successful initialization, a subsequent initialization error would not properly block processing, potentially allowing operations on a partially initialized model.
- Fixed an issue where toggling bypass mode or switching enhancement levels could produce discontinuities.

## 0.6.3 – 2025-08-22

- Integrates aic-sdk `v0.6.3`.
