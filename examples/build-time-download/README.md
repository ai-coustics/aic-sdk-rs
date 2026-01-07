# Build-Time Model Download Example

This example demonstrates how to download a model at build-time and embed it directly into your binary using the `include_model!` macro.

## Overview

The example shows a two-step process:

1. **Build Script (`build.rs`)**: Downloads the model during compilation and emits the path as an environment variable
2. **Application (`src/main.rs`)**: Uses the `include_model!` macro to embed the downloaded model directly into the binary

This approach is useful when you want to:
- Create a self-contained binary with the model included
- Avoid runtime model loading overhead
- Ensure the model is always available without network access at runtime

## How It Works

The `build.rs` script:
- Uses `Model::download()` to fetch the model at build-time
- Downloads to the `OUT_DIR` (Cargo's build output directory)
- Emits the model path as `MODEL_PATH` environment variable via `cargo:rustc-env`

The `main.rs` application:
- Reads the `MODEL_PATH` environment variable set by the build script
- Uses `include_model!(env!("MODEL_PATH"))` to embed the model bytes at compile-time
- The model is available as a static byte slice in the final binary

## Running the Example

```bash
cargo run --package build-time-download
```
