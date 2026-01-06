# AIC SDK Examples

This directory contains example code demonstrating how to use the AIC SDK Rust bindings.

## Prerequisites

Before running the examples, you need to:

1. Set your AIC SDK license key as an environment variable:
   ```bash
   export AIC_SDK_LICENSE=your_license_key_here
   ```

2. Ensure the AIC SDK C library is available (this should be handled automatically if you're using the `download-lib` feature).

## Running Examples

### Basic Usage Example

The `basic_usage` example demonstrates the core functionality of the AIC SDK, including:

- Displaying the SDK version
- Creating and initializing a model
- Getting optimal audio settings
- Setting and getting parameters
- Processing audio in both planar and interleaved formats
- Resetting the model

To run the example:

```bash
cargo run --example basic_usage --features download-lib
```

This example mirrors the C example that comes with the AIC SDK library, showing how to use the Rust API to perform the same operations in a safe, idiomatic way.

### Build-Time Model Download Example

The `build-time-download` example demonstrates how to download a model at build-time and embed it directly into your binary using the `include_model!` macro.

Key features:
- Downloads the model during compilation via `build.rs`
- Embeds the model bytes directly into the binary
- Creates a self-contained executable with no runtime model loading

To run the example:

```bash
cargo run --example build-time-download
```

See the [build-time-download/README.md](build-time-download/README.md) for more details on how the build-time download pattern works.

## Example Output

When successful, you should see output similar to:

```
Library version: 1.x.x
Model created successfully
Optimal sample rate: 48000 Hz
Optimal frame count: 480
Model initialized successfully
Output delay: 1440 samples
Parameter set successfully
Enhancement level: 0.7
Planar processing succeeded
Interleaved processing succeeded
Model reset succeeded
All tests completed
```

## Troubleshooting

- **License Error**: Make sure your `AIC_SDK_LICENSE` environment variable is set to a valid license key
- **Library Not Found**: Ensure you're using the `download-lib` feature or have the AIC SDK C library installed on your system
- **Processing Errors**: Check that the model was properly initialized before attempting to process audio
