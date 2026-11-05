### Breaking Changes

#### New model file version

This release requires model file version 8.
Re-download your models so the SDK does not reject them with `AicError::ModelVersionUnsupported`.
See the [compatibility matrix](https://docs.ai-coustics.com/reference/sdk/compatibility-matrix).

### New Features

- Quail Voice Focus 2.2 has been replaced by version 2.3, with model key `quail-vf-2.3-l-16khz` and `quail-vf-2.3-s-16khz`.
  Quail Voice Focus 2.2 (`quail-vf-2.2-l-16khz`/`quail-vf-2.2-s-16khz`) is not loadable by this SDK version any more.

- VAD Voice Focus 2.0 has been replaced by version 2.1, with model key `vad-vf-2.1-s-16khz`.
  VAD Voice Focus 2.0 (`vad-vf-2.0-s-16khz`) is not loadable by this SDK version any more.

- Tyto 1.1 has been replaced by version 1.2, with key `tyto-1.2-l-16khz`.
  Tyto 1.1 (`tyto-1.1-l-16khz`) is not loadable by this SDK version any more.

### Improvements

- The SDK now supports loading models with half-precision parameters (`f16`),
  improving inference speed on memory-bound models without sacrificing model performance.
- `aic-sdk-sys` now includes pre-generated bindings for all supported SDK targets. Building the Rust SDK no longer requires libclang, including when cross-compiling.
