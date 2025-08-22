# Rust Wrapper for the ai-coustics SDK

## Integration

Enable the `download-lib` feature to automatically download the library when building the crate.

```toml
[dependencies]
aic-sdk = { version = "0.6.2", features = ["download-lib"] }
```

If you want to provide your own library, use the `AIC_LIB_PATH` environment variable to specify the path
to the directory where the library is located.

## Example

```rust
use aic_sdk::{Model, ModelType, Parameter};

let license_key = std::env::var("AIC_SDK_LICENSE")?;

// Create a speech enhancement model by selecting a model type
// and passing your license key as an &str
let mut model = Model::new(ModelType::QuailS48, &license_key)?;

// Initialize the model with your audio settings
model.initialize(48000, 1, 480)?;

let mut audio_buffer = vec![0.0f32; 480];

// The process function is where the actual enhancement is happening
// This is meant to be called in your real-time audio thread
model.process_interleaved(&mut audio_buffer, 1, 480)?;

// You can also adjust parameters during processing
model.set_parameter(Parameter::EnhancementLevel, 0.8)?;

// For planar audio processing (separate channel buffers)
let mut audio = vec![vec![0.0f32; 480]; 2]; // 2 channels, 480 frames each
let mut audio_refs: Vec<&mut [f32]> = audio.iter_mut().map(|ch| ch.as_mut_slice()).collect();
model.initialize(48000, 2, 480)?;
model.process_planar(&mut audio_refs)?;
```

## Compatibility

This crate currently builds on Linux and macOS. Windows is not yet supported.
