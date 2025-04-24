# Rust Wrapper for the ai|coustics SDK

This example is using the Linux x86 built of our SDK.
If you need a different built, just exchange the `libaic.a` or `aic.lib` file in the `aic-sdk-sys/libs/` folder.
The library will be statically linked so it can be used in any Rust project.

## Integration

You can drop this folder in your project and just add it like this in your `Cargo.toml` file.

```toml
[dependencies]
aic-sdk = { path = "path/to/aic-sdk-rs" } # or whatever name you gave the folder
```

## Example

```Rust
let licence_key = std::env::var("AIC_SDK_LICENSE")?;

// To create a speech enhancement model, just select if you want to use `ModelL` or `ModelS`
// and pass your license key as an `&str`.
let mut aic = AicModel::new(AicModelType::ModelL, &licence_key)?;

// Initialize has to be called at least once before processing can start
// and everytime when the audio settings change.
aic.initialize(1, 48000, 512)?;

let mut buffer = vec![1.0; 512];

// The process function is where the actual enhancement is happening.
// This is meant to be called in your real-time audio thread.
aic.process_interleaved(&mut buffer, 1, 512)?;
```
