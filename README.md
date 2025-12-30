# ai-coustics Speech Enhancement SDK for Rust

> [!WARNING]
> You must use a Rust version different from beta 1.92, which was used to build the static libraries. A solution is currently in development.

## What is this SDK?

Our Speech Enhancement SDK delivers state-of-the-art audio processing capabilities, enabling you to enhance speech clarity and intelligibility in real-time.

## Quick Start

### Generate your SDK License Key

To use this SDK, you'll need to generate an **SDK license key** from our [Development Portal](https://developers.ai-coustics.io).

**Please note:** The SDK license key is different from our cloud API product. If you have an API license key for our cloud services, it won't work with the SDK - you'll need to create a separate SDK license key in the portal.

## Integration

Enable the `download-lib` feature to automatically download the library when building the crate.

```toml
[dependencies]
aic-sdk = { version = "1.0.0", features = ["download-lib"] }
```

If you want to provide your own library, use the `AIC_LIB_PATH` environment variable to specify the path
to the directory where the library is located.

## Example Usage

```rust,no_run
use aic_sdk::{Config, Model, Parameter, Processor};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let license_key = std::env::var("AIC_SDK_LICENSE")?;

    // Load a model file you already have on disk
    let model = Model::from_file("/path/to/model.aicmodel")?;

    // Create a processor using the model and your license
    let mut processor = Processor::new(&model, &license_key)?;

    // Initialize the processor with your audio settings
    let config = Config { num_channels: 2, allow_variable_frames: true, ..processor.optimal_config() };
    processor.initialize(&config)?;

    let mut audio_buffer = vec![0.0f32; config.num_frames * config.num_channels];

    // The process function is where the actual enhancement is happening
    // This is meant to be called in your real-time audio thread
    processor.process_interleaved(&mut audio_buffer)?;

    // You can also adjust parameters during processing
    processor.set_parameter(Parameter::EnhancementLevel, 0.8)?;

    // For planar audio processing (separate channel buffers)
    let mut audio = vec![vec![0.0f32; config.num_frames]; config.num_channels];
    let mut audio_refs: Vec<&mut [f32]> = audio.iter_mut().map(|ch| ch.as_mut_slice()).collect();
    processor.process_planar(&mut audio_refs)?;

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
cargo run --example basic_usage --features "download-lib,download-model"
```

## Support & Resources

### Documentation
- **[Basic Example](examples/basic_usage.rs)** - Sample code and integration patterns

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
