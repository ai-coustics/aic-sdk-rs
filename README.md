# aic-sdk - Rust Bindings for ai-coustics SDK

Rust wrapper for the ai-coustics Speech Enhancement SDK.

For comprehensive documentation, visit [docs.ai-coustics.com](https://docs.ai-coustics.com).

> [!NOTE]
> This SDK requires a license key. Generate your key at [developers.ai-coustics.com](https://developers.ai-coustics.com).

> [!WARNING]
> You must use a Rust version different from `1.93.0-beta.5`, which was used to build the static libraries. A solution is currently in development.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
aic-sdk = { version = "2.0.0", features = ["download-lib"] }
```

## Quick Start

```rust,ignore
use aic_sdk::{include_model, ProcessorConfig, Model, Processor};

// Embed model at compile time (or use Model::from_file to load at runtime)
static MODEL: &'static [u8] = include_model!("/path/to/model.aicmodel");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get your license key
    let license_key = std::env::var("AIC_SDK_LICENSE")?;

    // Load the embedded model (or download manually at https://artifacts.ai-coustics.io/)
    let model = Model::from_buffer(MODEL)?;
    
    // Get optimal configuration based on the selected model
    let config = ProcessorConfig::optimal(&model).with_num_channels(2);

    // Create a processor
    let mut processor = Processor::new(model, &license_key)?;

    // Processor needs to be initialized before processing
    processor.initialize(&config)?;

    // Process audio (interleaved: channels × frames)
    let mut audio_buffer = vec![0.0f32; config.num_channels as usize * config.num_frames];
    processor.process_interleaved(&mut audio_buffer)?;

    Ok(())
}
```

## Usage

### SDK Information

```rust
use aic_sdk;

// Get SDK version
println!("SDK version: {}", aic_sdk::get_sdk_version());

// Get compatible model version
println!("Compatible model version: {}", aic_sdk::get_compatible_model_version());
```

### Loading Models

Download models and find available IDs at [artifacts.ai-coustics.io](https://artifacts.ai-coustics.io/).

#### Load from File
```rust,ignore
use aic_sdk::Model;

let model = Model::from_file("path/to/model.aicmodel")?;
```

#### Embed at Compile Time
```rust,ignore
use aic_sdk::{Model, include_model};

static MODEL: &'static [u8] = include_model!("/path/to/model.aicmodel");
let model = Model::from_buffer(MODEL)?;
```

#### Download from CDN
Enable the `download-model` feature:
```toml
[dependencies]
aic-sdk = { version = "2.0.0", features = ["download-lib", "download-model"] }
```

```rust,ignore
use aic_sdk::Model;

let model_path = Model::download("quail-xxs-48khz", "./models")?;
let model = Model::from_file(&model_path)?;
```

### Model Information

```rust,ignore
// Get model ID
let model_id = model.get_id();

// Get optimal sample rate for the model
let optimal_rate = model.get_optimal_sample_rate();

// Get optimal frame count for a specific sample rate
let optimal_frames = model.get_optimal_num_frames(48000);
```

### Configuring the Processor

```rust,ignore
use aic_sdk::ProcessorConfig;

// Get optimal configuration for the model
let config = ProcessorConfig::optimal(&model)
    .with_num_channels(1)
    .with_allow_variable_frames(false);
println!("{:?}", config);  // ProcessorConfig { sample_rate: 48000, num_channels: 1, num_frames: 480, allow_variable_frames: false }

// Or create from scratch
let config = ProcessorConfig {
    sample_rate: 48000,
    num_channels: 2,
    num_frames: 480,
    allow_variable_frames: false,
};

// Processor needs to be initialized before processing
processor.initialize(&config)?;
```

### Processing Audio

```rust,ignore
// Interleaved processing (channels interleaved in single buffer)
// Format: [l, r, l, r, ...]
let mut audio_buffer = vec![0.0f32; config.num_channels as usize * config.num_frames];
processor.process_interleaved(&mut audio_buffer)?;

// Sequential processing (channels in sequence)
// Format: [l, l, ..., r, r, ...]
let mut audio_sequential = vec![0.0f32; config.num_channels as usize * config.num_frames];
processor.process_sequential(&mut audio_sequential)?;

// Planar processing (separate buffer per channel)
// Format: [[l, l, ...], [r, r, ...]]
let mut audio = vec![vec![0.0f32; config.num_frames]; config.num_channels as usize];
processor.process_planar(&mut audio)?;
```

### Processor Context

The processor context provides thread-safe access to processor parameters and state. You can create multiple contexts and move them to any thread for concurrent parameter updates.

```rust,ignore
use aic_sdk::ProcessorParameter;

// Get processor context
let proc_ctx = processor.processor_context();

// Get output delay in samples
let delay = proc_ctx.get_output_delay();

// Reset processor state (clears internal buffers)
proc_ctx.reset()?;

// Set enhancement parameters
proc_ctx.set_parameter(ProcessorParameter::EnhancementLevel, 0.8)?;
proc_ctx.set_parameter(ProcessorParameter::VoiceGain, 1.5)?;
proc_ctx.set_parameter(ProcessorParameter::Bypass, 0.0)?;

// Get parameter values
let level = proc_ctx.parameter(ProcessorParameter::EnhancementLevel)?;
println!("Enhancement level: {}", level);
```

### Voice Activity Detection (VAD)

The VAD context provides thread-safe access to VAD parameters and state. You can create multiple contexts and move them to any thread for concurrent parameter updates.

```rust,ignore
use aic_sdk::VadParameter;

// Get VAD context from processor
let vad_ctx = processor.vad_context();

// Configure VAD parameters
vad_ctx.set_parameter(VadParameter::Sensitivity, 6.0)?;
vad_ctx.set_parameter(VadParameter::SpeechHoldDuration, 0.05)?;
vad_ctx.set_parameter(VadParameter::MinimumSpeechDuration, 0.0)?;

// Get parameter values
let sensitivity = vad_ctx.parameter(VadParameter::Sensitivity)?;
println!("VAD sensitivity: {}", sensitivity);

// Check for speech (after processing audio through the processor)
if vad_ctx.is_speech_detected() {
    println!("Speech detected!");
}
```

## Examples

See the example files for complete working examples:
- [`examples/basic_usage.rs`](examples/basic_usage.rs) - Basic usage example
- [`examples/build-time-download`](examples/build-time-download) - Download and embed models at compile-time

Run examples with:
```bash
export AIC_SDK_LICENSE="your_license_key_here"
cargo run --example basic_usage --features download-lib,download-model
```

## Documentation

- **Full Documentation**: [docs.ai-coustics.com](https://docs.ai-coustics.com)
- **Rust API Reference**: [docs.rs/aic-sdk](https://docs.rs/aic-sdk)
- **Available Models**: [artifacts.ai-coustics.io](https://artifacts.ai-coustics.io)

## License

This Rust wrapper is distributed under the Apache 2.0 license. The core C SDK is distributed under the proprietary AIC-SDK license.
