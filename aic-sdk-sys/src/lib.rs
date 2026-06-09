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

    // This file contains an automatically generated file with the following code:
    // `aic_symbols! { list of fn declarations }`
    include!(concat!(env!("OUT_DIR"), "/runtime_linking.rs"));

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
