//! Raw FFI bindings to the AIC library.
//!
//! This module contains automatically generated bindings from the C header file.
//! The bindings are generated using bindgen and may contain naming conventions
//! that don't match Rust standards, which is expected for FFI code.
//!
//! # Runtime linking
//!
//! Enable the `runtime-linking` feature to load the AIC dynamic library at runtime instead of
//! linking to `libaic` at build time. The library is loaded automatically the first time any
//! `aic_*` function is called, using the platform's default name (`libaic.so`, `libaic.dylib`,
//! or `aic.dll`) resolved through the normal dynamic loader search path (`LD_LIBRARY_PATH`,
//! rpath, system directories, …).
//!
//! To load a specific file instead, call [`load_library`] with an explicit path before the first
//! SDK call:
//!
//! ```no_run
//! # #[cfg(feature = "runtime-linking")]
//! # unsafe fn example() -> Result<(), aic_sdk_sys::DynamicLoadingError> {
//! unsafe { aic_sdk_sys::load_library("/path/to/libaic.so")? };
//! # Ok(())
//! # }
//! ```

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(clippy::all)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(not(feature = "runtime-linking"))]
unsafe extern "C" {
    /// Sets the SDK wrapper ID.
    ///
    /// This function is not included in the C SDK's header file, but it is part of the library.
    pub fn aic_set_sdk_wrapper_id(id: u32);
}

#[cfg(feature = "runtime-linking")]
mod runtime_linking {
    use super::*;
    use libloading::Library;
    use std::{
        ffi::{c_char, c_float, c_uint},
        fmt,
        path::{Path, PathBuf},
        sync::OnceLock,
    };

    static AIC_LIBRARY: OnceLock<LoadedLibrary> = OnceLock::new();

    /// Error returned when loading the AIC dynamic library at runtime fails.
    #[derive(Debug)]
    pub enum DynamicLoadingError {
        /// A dynamic library was already loaded. The active library cannot be replaced safely.
        AlreadyLoaded,
        /// Opening the dynamic library failed.
        OpenLibrary {
            /// Path that was passed to [`load_library`].
            path: PathBuf,
            /// Error reported by the platform dynamic loader.
            source: libloading::Error,
        },
        /// A required AIC symbol could not be found in the dynamic library.
        LoadSymbol {
            /// Name of the missing symbol.
            symbol: &'static str,
            /// Error reported by the platform dynamic loader.
            source: libloading::Error,
        },
    }

    impl fmt::Display for DynamicLoadingError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::AlreadyLoaded => write!(f, "the AIC dynamic library is already loaded"),
                Self::OpenLibrary { path, source } => write!(
                    f,
                    "failed to load AIC dynamic library '{}': {source}",
                    path.display()
                ),
                Self::LoadSymbol { symbol, source } => {
                    write!(f, "failed to load AIC symbol '{symbol}': {source}")
                }
            }
        }
    }

    impl std::error::Error for DynamicLoadingError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match self {
                Self::AlreadyLoaded => None,
                Self::OpenLibrary { source, .. } | Self::LoadSymbol { source, .. } => Some(source),
            }
        }
    }

    struct LoadedLibrary {
        _library: Library,
        symbols: Symbols,
    }

    impl LoadedLibrary {
        unsafe fn load(path: &Path) -> Result<Self, DynamicLoadingError> {
            let library = unsafe { Library::new(path) }.map_err(|source| {
                DynamicLoadingError::OpenLibrary {
                    path: path.to_path_buf(),
                    source,
                }
            })?;
            let symbols = unsafe { Symbols::load(&library)? };

            Ok(Self {
                _library: library,
                symbols,
            })
        }
    }

    fn symbols() -> &'static Symbols {
        &AIC_LIBRARY
            .get_or_init(|| {
                // No explicit `load_library` happened, so load the platform's default `aic`
                // library by name and let the OS dynamic loader resolve it.
                let name = libloading::library_filename("aic");
                // SAFETY: opening the platform `aic` library; the operator is responsible for
                // making an ABI-compatible build discoverable on the loader search path.
                unsafe { LoadedLibrary::load(Path::new(&name)) }.unwrap_or_else(|err| {
                    panic!(
                        "{err}. Make it discoverable on the dynamic loader search path (e.g. \
                         LD_LIBRARY_PATH, rpath, or a system install), or call \
                         `aic_sdk_sys::load_library` with an explicit path before using the SDK."
                    )
                })
            })
            .symbols
    }

    macro_rules! aic_symbols {
        ($(
            fn $name:ident($($arg:ident: $arg_ty:ty),* $(,)?) $(-> $ret:ty)?;
        )+) => {
            struct Symbols {
                $(
                    $name: unsafe extern "C" fn($($arg_ty),*) $(-> $ret)?,
                )+
            }

            impl Symbols {
                unsafe fn load(library: &Library) -> Result<Self, DynamicLoadingError> {
                    $(
                        let $name = *unsafe {
                            library.get(concat!(stringify!($name), "\0").as_bytes())
                        }
                        .map_err(|source| DynamicLoadingError::LoadSymbol {
                            symbol: stringify!($name),
                            source,
                        })?;
                    )+

                    Ok(Self { $($name,)+ })
                }
            }

            $(
                pub unsafe fn $name($($arg: $arg_ty),*) $(-> $ret)? {
                    unsafe { (symbols().$name)($($arg),*) }
                }
            )+
        };
    }

    // This table mirrors the function declarations in `include/aic.h` (plus the headerless
    // `aic_set_sdk_wrapper_id`). It is maintained by hand because bindgen's generated `extern`
    // declarations are blocklisted in the `runtime-linking` mode. Whenever `aic.h` changes, update the signatures
    // below to match — a wrong signature compiles but is undefined behavior at call time. The
    // `check-header` CI job fails when `aic.h` drifts from the SDK release, and the `linking` CI
    // job runs the `basic_usage` example against a real `libaic` to exercise these symbols.
    aic_symbols! {
        fn aic_get_sdk_version() -> *const c_char;
        fn aic_get_compatible_model_version() -> c_uint;
        fn aic_model_create_from_file(model: *mut *mut AicModel, file_path: *const c_char) -> AicErrorCode::Type;
        fn aic_model_create_from_buffer(model: *mut *mut AicModel, buffer: *const u8, buffer_len: usize) -> AicErrorCode::Type;
        fn aic_model_destroy(model: *mut AicModel);
        fn aic_model_get_id(model: *const AicModel) -> *const c_char;
        fn aic_model_get_optimal_sample_rate(model: *const AicModel, sample_rate: *mut c_uint) -> AicErrorCode::Type;
        fn aic_model_get_optimal_num_frames(model: *const AicModel, sample_rate: c_uint, num_frames: *mut usize) -> AicErrorCode::Type;
        fn aic_processor_create(processor: *mut *mut AicProcessor, model: *const AicModel, license_key: *const c_char, otel_config: *const AicOtelConfig) -> AicErrorCode::Type;
        fn aic_processor_destroy(processor: *mut AicProcessor);
        fn aic_processor_initialize(processor: *mut AicProcessor, sample_rate: c_uint, num_channels: u16, num_frames: usize, allow_variable_frames: bool) -> AicErrorCode::Type;
        fn aic_processor_process_planar(processor: *mut AicProcessor, audio: *const *mut c_float, num_channels: u16, num_frames: usize) -> AicErrorCode::Type;
        fn aic_processor_process_interleaved(processor: *mut AicProcessor, audio: *mut c_float, num_channels: u16, num_frames: usize) -> AicErrorCode::Type;
        fn aic_processor_process_sequential(processor: *mut AicProcessor, audio: *mut c_float, num_channels: u16, num_frames: usize) -> AicErrorCode::Type;
        fn aic_processor_context_create(context: *mut *mut AicProcessorContext, processor: *const AicProcessor) -> AicErrorCode::Type;
        fn aic_processor_context_destroy(context: *mut AicProcessorContext);
        fn aic_processor_context_reset(context: *const AicProcessorContext) -> AicErrorCode::Type;
        fn aic_processor_context_set_parameter(context: *const AicProcessorContext, parameter: AicProcessorParameter::Type, value: c_float) -> AicErrorCode::Type;
        fn aic_processor_context_get_parameter(context: *const AicProcessorContext, parameter: AicProcessorParameter::Type, value: *mut c_float) -> AicErrorCode::Type;
        fn aic_processor_context_get_output_delay(context: *const AicProcessorContext, delay: *mut usize) -> AicErrorCode::Type;
        fn aic_processor_context_update_bearer_token(context: *const AicProcessorContext, token: *const c_char) -> AicErrorCode::Type;
        fn aic_vad_context_create(context: *mut *mut AicVadContext, processor: *const AicProcessor) -> AicErrorCode::Type;
        fn aic_vad_context_destroy(context: *mut AicVadContext);
        fn aic_vad_context_is_speech_detected(context: *const AicVadContext, value: *mut bool) -> AicErrorCode::Type;
        fn aic_vad_context_set_parameter(context: *const AicVadContext, parameter: AicVadParameter::Type, value: c_float) -> AicErrorCode::Type;
        fn aic_vad_context_get_parameter(context: *const AicVadContext, parameter: AicVadParameter::Type, value: *mut c_float) -> AicErrorCode::Type;
        fn aic_analyzer_pair_create(collector: *mut *mut AicCollector, analyzer: *mut *mut AicAnalyzer, model: *const AicModel, license_key: *const c_char, analysis_window_length_ms: usize) -> AicErrorCode::Type;
        fn aic_collector_initialize(collector: *mut AicCollector, sample_rate: c_uint, num_channels: u16, num_frames: usize, allow_variable_frames: bool) -> AicErrorCode::Type;
        fn aic_collector_buffer_planar(collector: *mut AicCollector, audio: *const *const c_float, num_channels: u16, num_frames: usize) -> AicErrorCode::Type;
        fn aic_collector_buffer_interleaved(collector: *mut AicCollector, audio: *const c_float, num_channels: u16, num_frames: usize) -> AicErrorCode::Type;
        fn aic_collector_buffer_sequential(collector: *mut AicCollector, audio: *const c_float, num_channels: u16, num_frames: usize) -> AicErrorCode::Type;
        fn aic_analyzer_reset(analyzer: *const AicAnalyzer) -> AicErrorCode::Type;
        fn aic_analyzer_analyze_buffered(analyzer: *mut AicAnalyzer, result: *mut AicAudioInsights) -> AicErrorCode::Type;
        fn aic_analyzer_update_bearer_token(analyzer: *const AicAnalyzer, token: *const c_char) -> AicErrorCode::Type;
        fn aic_collector_destroy(collector: *mut AicCollector);
        fn aic_analyzer_destroy(analyzer: *mut AicAnalyzer);
        fn aic_set_sdk_wrapper_id(id: c_uint);
    }

    /// Loads the AIC dynamic library from an explicit `path`.
    ///
    /// This is **optional**: if it is never called, the library is loaded automatically on first
    /// use from the platform's default name (`libaic.so` / `libaic.dylib` / `aic.dll`) via the OS
    /// loader search path. Call this when you need to choose the exact file instead.
    ///
    /// It must run before the first SDK call; once the library is loaded (explicitly or
    /// automatically) it is kept for the rest of the process so function pointers and SDK-owned
    /// objects stay valid, and a later call returns [`DynamicLoadingError::AlreadyLoaded`].
    ///
    /// # Safety
    ///
    /// `path` must point to a dynamic library that is ABI-compatible with the bundled `aic.h`
    /// header used to build these bindings. Loading an incompatible library can cause undefined
    /// behavior when its functions are called.
    pub unsafe fn load_library<P: AsRef<Path>>(path: P) -> Result<(), DynamicLoadingError> {
        let loaded = unsafe { LoadedLibrary::load(path.as_ref())? };
        AIC_LIBRARY
            .set(loaded)
            .map_err(|_| DynamicLoadingError::AlreadyLoaded)
    }

    /// Returns whether an AIC dynamic library has already been loaded.
    pub fn is_library_loaded() -> bool {
        AIC_LIBRARY.get().is_some()
    }
}

#[cfg(feature = "runtime-linking")]
pub use runtime_linking::*;
