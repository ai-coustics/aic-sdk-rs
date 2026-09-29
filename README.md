# aic-sdk - Rust Bindings for ai-coustics SDK

Rust wrapper for the ai-coustics SDK.

> [!NOTE]
> This SDK requires a license key. Generate your key at [developers.ai-coustics.com](https://developers.ai-coustics.com).

## Installation

```bash
cargo add aic-sdk --features download-lib
```

`download-lib` fetches the matching native library during the build.

## Quick Start

```rust,ignore
use aic_sdk::{include_model, ProcessorConfig, Model, Processor};

// Embed model at compile time (or use Model::from_file to load at runtime)
static MODEL: &[u8] = include_model!("/path/to/model.aicmodel");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get your license key from the environment variable
    let license_key = std::env::var("AIC_SDK_LICENSE")?;

    // Load the embedded model (or download manually at https://artifacts.ai-coustics.io/)
    let model = Model::from_buffer(MODEL)?;

    // Get optimal configuration based on the selected model
    let config = ProcessorConfig::optimal(&model);

    // Create a processor and initialize it
    let mut processor = Processor::new(&model, &license_key)?.with_config(&config)?;

    // Process mono audio
    let mut audio_block = vec![0.0f32; config.block_size];
    processor.process(&mut audio_block)?;

    Ok(())
}
```

## Documentation

The full documentation is the API reference at [docs.rs/aic-sdk](https://docs.rs/aic-sdk), which
also carries the long-form material:

- **`docs::guide`** covers loading models, configuring a processor, voice activity detection, the
  analyzer and async processing.
- **`docs::examples`** shows example programs that use the SDK.
- **`docs::linking`** covers static, dynamic and runtime linking of the native library.

Available models are listed at [artifacts.ai-coustics.io](https://artifacts.ai-coustics.io), and
the product documentation lives at [docs.ai-coustics.com](https://docs.ai-coustics.com).

## License

This Rust wrapper is distributed under the Apache 2.0 license.
The core C SDK is distributed under the proprietary AIC-SDK license.

`NOTICE.txt` lists the third-party software distributed with the SDK.

## Energy-based Voice Activity Detection

An enhancement processor can report speech activity without running a separate VAD model.
Create a context before processing and read its prediction after each audio block:

```rust,ignore
use aic_sdk::VadParameter;

let vad = processor.energy_vad_context();
vad.set_parameter(VadParameter::Sensitivity, 6.0)?;
processor.process(&mut audio)?;
let speech_detected = vad.is_speech_detected();
```

`EnergyVadContext` also provides `parameter`, `prediction_delay`, and `reset`.
It can be shared across threads and outlive the processor. Resetting it retains its
settings without resetting enhancement; resetting the processor also resets its energy VAD.
Sensitivity ranges from 1 to 15, with higher values detecting quieter signals.
Creating a context keeps inference active even while bypassed or fully dry.
With the `async` feature, use `processor.energy_vad_context().await` on `ProcessorAsync`.

## Crates

| Crate | crates.io | Documentation |
| --- | --- | --- |
| `aic-sdk` | <https://crates.io/crates/aic-sdk/0.25.0> | <https://docs.rs/aic-sdk/0.25.0> |
| `aic-sdk-sys` | <https://crates.io/crates/aic-sdk-sys/0.25.0> | <https://docs.rs/aic-sdk-sys/0.25.0> |
| `aic-model-downloader` | <https://crates.io/crates/aic-model-downloader/0.25.0> | <https://docs.rs/aic-model-downloader/0.25.0> |

## Source Code

The full source of these bindings ships inside the published crates. Read it online:

- <https://docs.rs/crate/aic-sdk/0.25.0/source/>
- <https://docs.rs/crate/aic-sdk-sys/0.25.0/source/>
- <https://docs.rs/crate/aic-model-downloader/0.25.0/source/>

Or download and unpack it:

```bash
curl -L https://static.crates.io/crates/aic-sdk/aic-sdk-0.25.0.crate | tar -xz
curl -L https://static.crates.io/crates/aic-sdk-sys/aic-sdk-sys-0.25.0.crate | tar -xz
curl -L https://static.crates.io/crates/aic-model-downloader/aic-model-downloader-0.25.0.crate | tar -xz
```
