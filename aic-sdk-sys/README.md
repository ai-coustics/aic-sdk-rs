# aic-sdk-sys

Unsafe Rust bindings for the ai|coustics SDK.

## Generating Bindings

The Rust bindings are generated from the original C++ header file.

To generate the bindings, use the following command:

```bash
bindgen aic.h --output aic.rs -- -x c++ -std=c++17
```

### Installing Bindgen

You can install `bindgen` via `cargo` using the following command:

```bash
cargo install bindgen-cli
```
