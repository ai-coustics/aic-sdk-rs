# ai-coustics Speech Enhancement SDK for Rust

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
aic-sdk = { version = "0.9.0", features = ["download-lib"] }
```

If you want to provide your own library, use the `AIC_LIB_PATH` environment variable to specify the path
to the directory where the library is located.

## Example Usage

```rust
use aic_sdk::{Model, ModelType, Parameter};

let license_key = std::env::var("AIC_SDK_LICENSE")?;

// Create a speech enhancement model by selecting a model type
// and passing your license key as an &str
let mut model = Model::new(ModelType::QuailS48, &license_key)?;

// Initialize the model with your audio settings
model.initialize(48000, 1, 480, true)?;

let mut audio_buffer = vec![0.0f32; 480];

// The process function is where the actual enhancement is happening
// This is meant to be called in your real-time audio thread
model.process_interleaved(&mut audio_buffer, 1, 480)?;

// You can also adjust parameters during processing
model.set_parameter(Parameter::EnhancementLevel, 0.8)?;

// For planar audio processing (separate channel buffers)
let mut audio = vec![vec![0.0f32; 480]; 2]; // 2 channels, 480 frames each
let mut audio_refs: Vec<&mut [f32]> = audio.iter_mut().map(|ch| ch.as_mut_slice()).collect();
model.initialize(48000, 2, 480, true)?;
model.process_planar(&mut audio_refs)?;
```

## Running the Example

To run the example, make sure you have set your license key as an environment variable:

```bash
export AIC_SDK_LICENSE="your_license_key_here"
```

Then use the following commands to configure, build and run the example:

```sh
cargo run --example basic_usage --features download-lib
```

## Compatibility

This crate currently builds on Linux and macOS. Windows is not yet supported.

If you need Windows support, please take a look at our C/C++ SDKs for now.

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
