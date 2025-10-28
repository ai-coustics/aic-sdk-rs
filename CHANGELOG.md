# Changelog

## 0.8.0 - 2025-10-28

### New features
- **Self-Service Licenses**: Starting with this release, you can use self-service licenses directly from our development portal.
- **Usage-Based Telemetry**: This release introduces a new telemetry feature that collects usage data, paving the way for future usage-based pricing models such as pay-per-minute billing.
    - **What we collect**: We collect only the processing time used and some diagnostic data
    - **Privacy**: We do not collect any information about your audio content. Your audio never leaves your device during our processing.
    - **Requirements**: Requires a constant internet connection. If the SDK cannot be activated online, enhancement will stop after 10 seconds. If telemetry data cannot be sent, enhancement will stop after 5 minutes. When enhancement is stopped an error will be returned, the audio will be bypassed and the processing delay will be still applied to ensure an uninterrupted audio stream without discontinuities.
    - **Error Handling**: When processing is bypassed because our backend cannot be reached or does not allow you to process, the process functions will return AIC_ERROR_CODE_ENHANCEMENT_NOT_ALLOWED. Make sure to handle this error code in your implementation.
    - **Offline Licenses**: If you cannot provide a constant internet connection, please contact us to obtain a special offline license that does not require telemetry.


### Breaking changes
- **Variable number of frames supported**: `Model::initialize` now supports a variable number of frames per call. To enable this feature, use the new `allow_variable_frames` parameter in the initialize function. Set allow_variable_frames to true to enable variable frame processing, or false to maintain the previous fixed frame behavior. Note that enabling variable frames results in higher processing delay.
- **New bypass parameter**: A new parameter `Parameter::Bypass` has been added to control audio processing bypass while preserving algorithmic delay. When enabled, the input audio passes through unmodified, but the output is still delayed by the same amount as during normal processing. This ensures seamless transitions when toggling enhancement on/off without audible clicks or timing shifts.
- **Updated Error Codes**: Expanded and renamed error variants, with additional license-related errors.

### Fixes
- The internal model state is now automatically reset when processing is paused (e.g., when bypass is enabled or enhancement level is set to 0). This ensures a clean state when processing resumes.
- The reset operation now ensures that all internal DSP components are properly reset, providing a more thorough clean state.
- Fixed an issue where, after a successful initialization, a subsequent initialization error would not properly block processing, potentially allowing operations on a partially initialized model.
- Fixed an issue where toggling bypass mode or switching enhancement levels could produce discontinuities.

## 0.6.3 – 2025-08-22

- Integrates aic-sdk `v0.6.3`.
