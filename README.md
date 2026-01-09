# ai-coustics Speech Enhancement SDK for Rust

> [!WARNING]
> You must use a Rust version different from beta 1.92, which was used to build the static libraries. A solution is currently in development.

## What is this SDK?

Our Speech Enhancement SDK delivers state-of-the-art audio processing capabilities, enabling you to enhance speech clarity and intelligibility in real-time.

## Quick Start

### Generate your SDK License Key

To use this SDK, you'll need to generate an **SDK license key** from our [Development Portal](https://developers.ai-coustics.io).

**Please note:** The SDK license key is different from our cloud API product. If you have an API license key for our cloud services, it won't work with the SDK - you'll need to create a separate SDK license key in the portal.

### Download a Model

Models can be obtained in two ways:

1. **Manual Download**: Visit [artifacts.ai-coustics.io](https://artifacts.ai-coustics.io/) to browse and download models directly. Once downloaded, you can load the model file:
   - At runtime using `Model::from_file`
   - At compile-time using the `include_model!` macro

2. **Programmatic Download**: Enable the `download-model` feature to use the `Model::download` function, which fetches models by their ID from [artifacts.ai-coustics.io](https://artifacts.ai-coustics.io/).

## Integration

### Library

Enable the `download-lib` feature to automatically download the library when building the crate.

```toml
[dependencies]
aic-sdk = { version = "2.0.0", features = ["download-lib"] }
```

If you want to provide your own library, use the `AIC_LIB_PATH` environment variable to specify the path
to the directory where the library is located.

### Models

Enable the `download-model` feature to enable the `Model::download` API.

```toml
[dependencies]
aic-sdk = { version = "2.0.0", features = ["download-model"] }
```

Our models are available for download at [artifacts.ai-coustics.io](https://artifacts.ai-coustics.io).
We recommend that the selected model is downloaded and embedded into your binary using the `include_model` macro.

## Example Usage

```rust,ignore
use aic_sdk::{include_model, ProcessorConfig, Model, Processor, ProcessorParameter};

static MODEL: &'static [u8] = include_model!("/path/to/model.aicmodel");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let license_key = std::env::var("AIC_SDK_LICENSE")?;

    // Load the embedded model
    let model = Model::from_buffer(MODEL)?;

    // Create a processor using the model and your license
    let mut processor = Processor::new(&model, &license_key)?;

    // Set up your desired audio settings
    let config = ProcessorConfig {
        num_channels: 2,
        allow_variable_frames: true,
        ..ProcessorConfig::optimal(&model)
    };

    // Initialize the processor
    processor.initialize(&config)?;

    let mut audio_buffer = vec![0.0f32; config.num_channels as usize * config.num_frames];

    // The process function is where the actual enhancement is happening
    // This is meant to be called in your real-time audio thread
    processor.process_interleaved(&mut audio_buffer)?;
    
    let processor_context = processor.processor_context();

    // You can also adjust parameters during processing
    processor_context.set_parameter(ProcessorParameter::EnhancementLevel, 0.8)?;

    // For planar audio processing (separate channel buffers)
    let mut audio = vec![vec![0.0f32; config.num_frames]; config.num_channels as usize];
    processor.process_planar(&mut audio)?;

    Ok(())
}
```

## Running the Example

To run the example, make sure you have set your license key as an environment variable:

```bash
export AIC_SDK_LICENSE="your_license_key_here"
```

Then use the following commands to configure, build and run the example:

```sh
cargo run --example basic_usage --features download-lib,download-model
```

To run the example package that shows how to download and embed models at compiletime run:

```sh
cargo run --package build-time-download --features download-lib
```

## Support & Resources

### Documentation
- **[Basic Example](examples/basic_usage.rs)** - Sample code and integration patterns
- **[Build-Time Download Example](examples/build-time-download)** - Download and embed models at compile-time

### Looking for Other Languages?
The ai-coustics Speech Enhancement SDK is available in multiple programming languages to fit your development needs:

| Platform | Repository | Description |
|----------|------------|-------------|
| **C** | [`aic-sdk-c`](https://github.com/ai-coustics/aic-sdk-c) | Core C interface and foundation library |
| **C++** | [`aic-sdk-cpp`](https://github.com/ai-coustics/aic-sdk-cpp) | Modern C++ interface with RAII and type safety |
| **Python** | [`aic-sdk-py`](https://github.com/ai-coustics/aic-sdk-py) | Idiomatic Python interface |
| **JavaScript/TypeScript** | [`aic-sdk-node`](https://github.com/ai-coustics/aic-sdk-node) | Native bindings for Node.js applications |
| **Web (WASM)** | [`aic-sdk-wasm`](https://github.com/ai-coustics/aic-sdk-wasm) | WebAssembly build for browser applications |

All SDKs provide the same core functionality with language-specific optimizations and idioms.

### Get Help
Need assistance? We're here to support you:
- **Issues**: [GitHub Issues](https://github.com/ai-coustics/aic-sdk-rs/issues)
- **Technical Support**: [info@ai-coustics.com](mailto:info@ai-coustics.com)

## License
This Rust wrapper is distributed under the `Apache 2.0 license`, while the core C SDK is distributed under the proprietary `AIC-SDK license`.

---

Made with ❤️ by the ai-coustics team
