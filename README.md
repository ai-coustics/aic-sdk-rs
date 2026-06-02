# aic-sdk - Rust Bindings for ai-coustics SDK

Rust wrapper for the ai-coustics Speech Enhancement SDK.

For comprehensive documentation, visit [docs.ai-coustics.com](https://docs.ai-coustics.com).

> [!NOTE]
> This SDK requires a license key. Generate your key at [developers.ai-coustics.com](https://developers.ai-coustics.com).

> [!WARNING]
> The bundled libraries were built with Rust `1.97.0-beta.1`. Building your crate with that exact toolchain version fails to link, so use any other Rust version. This affects the default static linking only — the `dynamic-linking` and `runtime-linking` modes are unaffected (see [Linking the native SDK](#linking-the-native-sdk)).

## Installation

Add to your project:

```bash
cargo add aic-sdk --features download-lib
```

## Quick Start

```rust,ignore
use aic_sdk::{include_model, ProcessorConfig, Model, Processor};

// Embed model at compile time (or use Model::from_file to load at runtime)
static MODEL: &'static [u8] = include_model!("/path/to/model.aicmodel");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get your license key from the environment variable
    let license_key = std::env::var("AIC_SDK_LICENSE")?;

    // Load the embedded model (or download manually at https://artifacts.ai-coustics.io/)
    let model = Model::from_buffer(MODEL)?;

    // Get optimal configuration based on the selected model
    let config = ProcessorConfig::optimal(&model).with_num_channels(2);

    // Create a processor and initialize it
    let mut processor = Processor::new(&model, &license_key)?.with_config(&config)?;

    // Process audio (interleaved: channels × frames)
    let mut audio_buffer = vec![0.0f32; config.num_channels as usize * config.num_frames];
    processor.process_interleaved(&mut audio_buffer)?;

    Ok(())
}
```

## Linking the native SDK

By default, `aic-sdk-sys` links the native AIC SDK **statically**. Two opt-in features link a shared `libaic` instead. They are mutually exclusive; enabling both (e.g. via `--all-features`) selects `runtime-linking`.

| Feature | Linking | How `libaic` is located |
|---|---|---|
| _(default)_ | static, at build time | `AIC_LIB_PATH` directory, or downloaded with `download-lib` |
| `dynamic-linking` | dynamic, at build time | same to link; then OS loader search at run time |
| `runtime-linking` | dynamic, lazy on first use | OS loader search by name, or `aic_sdk::load_library(path)` |

In every mode, point the build at the SDK with `AIC_LIB_PATH=/path/to/aic-sdk/lib`, or enable `download-lib` to fetch it automatically:

```bash
AIC_SDK_LICENSE="…" cargo run --example basic_usage \
  --features "dynamic-linking download-lib download-model"
```

> [!NOTE]
> The `dynamic-linking` and `runtime-linking` modes are not affected by the toolchain link issue in the [warning above](#aic-sdk---rust-bindings-for-ai-coustics-sdk); it applies to default static linking only.

### Finding the library at run time

With `dynamic-linking` and `runtime-linking`, the OS dynamic loader must locate `libaic` when the program runs — `download-lib` only covers build time. Point the loader at the SDK `lib` directory, ship the library next to the binary, or install it system-wide:

- **Linux:** `LD_LIBRARY_PATH=/path/to/aic-sdk/lib`, or an rpath (`RUSTFLAGS="-C link-arg=-Wl,-rpath,\$ORIGIN"` + ship `libaic.so` beside the binary).
- **macOS:** `DYLD_LIBRARY_PATH`, `@rpath`/`@loader_path`, or a bundle layout.
- **Windows:** put `aic.dll` next to the `.exe` or on `PATH` (the build-time import lib `aic.lib` and the runtime `aic.dll` may be in different directories).
- **Android:** package `libaic.so` (arm64 only) into the APK under `lib/arm64-v8a/`.

`runtime-linking` loads `libaic` automatically on the first SDK call, by platform default name (`libaic.so` / `libaic.dylib` / `aic.dll`). To choose an exact file, call `load_library` first; if the library can't be found, that first call panics with a descriptive message:

```rust,ignore
unsafe { aic_sdk::load_library("/path/to/libaic.so")?; } // optional override
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

```bash
cargo add aic-sdk --features download-lib,download-model
```

```rust,ignore
use aic_sdk::Model;

let model_path = Model::download("quail-vf-2.1-s-16khz", "./models")?;
let model = Model::from_file(&model_path)?;
```

### Model Information

```rust,ignore
// Get model ID
let model_id = model.id();

// Get optimal sample rate for the model
let optimal_rate = model.optimal_sample_rate();

// Get optimal frame count for a specific sample rate
let optimal_frames = model.optimal_num_frames(48000);
```

### Configuring the Processor

```rust,ignore
use aic_sdk::{Processor, ProcessorConfig};

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

// Option 1: Create and initialize in one step
let processor = Processor::new(&model, &license_key)?.with_config(&config)?;

// Option 2: Create first, then initialize separately
let mut processor = Processor::new(&model, &license_key)?;
processor.initialize(&config)?;
```

### OpenTelemetry

By default, processor telemetry follows the SDK environment configuration, such as
`AIC_SDK_OTEL_ENABLE`. Use `OtelConfig` when a single processor needs an explicit
telemetry setting or session ID.

```rust,ignore
use aic_sdk::{OtelConfig, Processor};

let otel = OtelConfig::with_session_id("session-1");
let processor = Processor::with_otel_config(&model, &license_key, &otel)?
    .with_config(&config)?;
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
let delay = proc_ctx.output_delay();

// Reset processor state (clears internal buffers)
proc_ctx.reset()?;

// Set enhancement parameters
proc_ctx.set_parameter(ProcessorParameter::EnhancementLevel, 0.8)?;
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

### Async Processing

Enable the `async` feature to use [`ProcessorAsync`], which offloads
processing to a background thread pool and returns a future. The implementation
is runtime-agnostic and works on any executor (tokio, smol, async-std, ...).
The pool defaults to one thread per logical CPU; override with the
`AIC_NUM_THREADS` environment variable.

```bash
cargo add aic-sdk --features async
```

```rust,ignore
use aic_sdk::{Model, ProcessorAsync, ProcessorConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let license_key = std::env::var("AIC_SDK_LICENSE")?;
    let model = Model::from_file("path/to/model.aicmodel")?;
    let config = ProcessorConfig::optimal(&model).with_num_channels(2);

    let processor = ProcessorAsync::new(&model, &license_key)?
        .with_config(&config)
        .await?;

    // The async API takes ownership of the buffer and returns it back.
    let audio = vec![0.0f32; config.num_channels as usize * config.num_frames];
    let audio = processor.process_interleaved(audio).await?;
    Ok(())
}
```

## Examples

See the example files for complete working examples:

- [`examples/basic_usage.rs`](examples/basic_usage.rs) - Basic usage example
- [`examples/build-time-download`](examples/build-time-download) - Download and embed models at compile-time
- [`examples/benchmark.rs`](examples/benchmark.rs) - Run multiple processor instances concurrently until the real-time requirements are not met
- [`examples/parallel_async.rs`](examples/parallel_async.rs) - Async processing with `ProcessorAsync` across multiple instances (requires `async`)

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
