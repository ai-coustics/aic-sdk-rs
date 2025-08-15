//! Raw FFI bindings to the AIC library.
//!
//! This module contains automatically generated bindings from the C header file.
//! The bindings are generated using bindgen and may contain naming conventions
//! that don't match Rust standards, which is expected for FFI code.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(clippy::all)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
