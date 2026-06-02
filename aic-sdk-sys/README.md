# aic-sdk-sys

Unsafe Rust bindings for the ai-coustics SDK.

## Linking modes

By default, this crate links the native AIC SDK statically.

`dynamic-linking` and `runtime-linking` are mutually exclusive linking strategies — enable at most one. Because Cargo features are additive, enabling both (for example via `--all-features`) is allowed but selects `runtime-linking`.

### Static linking

Set `AIC_LIB_PATH` to the directory containing the native static library, or enable `download-lib` to download the SDK during the build.

```bash
AIC_LIB_PATH=/path/to/aic-sdk/lib cargo build -p aic-sdk-sys
```

### Compile-time dynamic linking

Enable `dynamic-linking` to link against `libaic.so` / `libaic.dylib` / `aic.dll` dynamically at build time.

If `download-lib` is also enabled and `AIC_LIB_PATH` is not set, the native SDK is downloaded automatically and used for build-time linking:

```bash
cargo build -p aic-sdk-sys --features "dynamic-linking download-lib"
```

Alternatively, set `AIC_LIB_PATH` to a local SDK `lib` directory:

```bash
AIC_LIB_PATH=/path/to/aic-sdk/lib cargo build -p aic-sdk-sys --features dynamic-linking
```

At runtime, the operating system dynamic loader must also be able to find the library. `download-lib` solves build-time discovery, but it does not configure runtime loader paths. On Linux, for local testing with a local SDK directory:

```bash
AIC_LIB_PATH=/path/to/aic-sdk/lib \
LD_LIBRARY_PATH=/path/to/aic-sdk/lib \
cargo run --example basic_usage --features "dynamic-linking download-model"
```

If you copy or move the compiled Linux binary, also make `libaic.so` available at runtime:

```bash
LD_LIBRARY_PATH=/path/to/aic-sdk/lib ./basic_usage
```

On Windows, `LD_LIBRARY_PATH` does not apply. Put `aic.dll` next to the executable or add the directory containing `aic.dll` to `PATH`:

```powershell
$env:PATH = "C:\path\to\aic-sdk\lib;$env:PATH"
.\basic_usage.exe
```

Depending on the SDK package layout, the import library used at build time (`aic.lib`) and the runtime DLL (`aic.dll`) may be in different directories. `AIC_LIB_PATH` is for the build linker; `PATH` is for the runtime DLL loader.

### Runtime dynamic loading

Enable `runtime-linking` to skip native build-time linking. The library is loaded automatically on first use from the platform's default name (`libaic.so` / `libaic.dylib` / `aic.dll`) via the OS loader search path, so it only needs to be discoverable (e.g. through `LD_LIBRARY_PATH`, rpath, or a system install).

To load a specific file instead, call `load_library` with its full path before the first SDK call:

```rust,ignore
unsafe {
    aic_sdk_sys::load_library("/path/to/libaic.so")?;
}
```

If the library cannot be located (neither by the loader nor via `load_library`), the first `aic_*` call **panics** with a descriptive message.
