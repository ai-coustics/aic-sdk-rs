# Build-Time Model Download Example

This example demonstrates how to download a model at build-time and embed it directly into your binary using the `include_model!` macro.

## Overview

The example shows a two-step process:

1. **Build Script (`build.rs`)**: Downloads the model during compilation and emits the path as an environment variable
2. **Application (`src/main.rs`)**: Uses the `include_model!` macro to embed the downloaded model directly into the binary

## Running the Example

```bash
cargo run --package build-time-download --features download-lib
```
