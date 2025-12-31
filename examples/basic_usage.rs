#![cfg_attr(not(feature = "download-model"), allow(dead_code, unused_imports))]

#[cfg(feature = "download-model")]
use aic_sdk::{Config, Model, Parameter, Processor, VadParameter};
use std::env;

#[cfg(not(feature = "download-model"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err("Enable the `download-model` feature to run this example.".into())
}

#[cfg(feature = "download-model")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Display library version
    println!("ai-coustics SDK version: {}", aic_sdk::get_version());

    // Get license key from environment variable
    let license = env::var("AIC_SDK_LICENSE").map_err(|_| {
        eprintln!("Error: AIC_SDK_LICENSE environment variable not set");
        eprintln!("Please set it with: export AIC_SDK_LICENSE=your_license_key");
        std::io::Error::new(std::io::ErrorKind::NotFound, "AIC_SDK_LICENSE not set")
    })?;

    // Download the default model once and reuse the file
    let target_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target");
    let model_path = Model::download("quail-xxs-48khz", target_dir)?;
    let model = Model::from_file(&model_path)?;
    println!("Model loaded from {}", model_path.display());

    // Create processor with license key
    let mut processor = Processor::new(&model, &license)?;
    println!("Processor created successfully");

    // Set up configuration
    let config = Config {
        num_channels: 2,
        allow_variable_frames: true,
        ..processor.optimal_config()
    };

    // Initialize the processor
    processor.initialize(&config)?;
    println!(
        "Processor initialized: Sample rate: {} Hz, Frames: {}, Channels: {}",
        config.sample_rate, config.num_frames, config.num_channels
    );

    // Get output delay
    let delay = processor.output_delay();
    println!("Output delay: {} samples", delay);

    // Test parameter setting and getting
    processor.set_parameter(Parameter::EnhancementLevel, 0.7)?;
    println!("Parameter set successfully");

    let enhancement_level = processor.parameter(Parameter::EnhancementLevel)?;
    println!("Enhancement level: {}", enhancement_level);

    // Create minimal test audio - planar format (separate buffers for each channel)
    let mut audio_buffer_left = vec![0.0f32; config.num_frames];
    let mut audio_buffer_right = vec![0.0f32; config.num_frames];

    // Create mutable references for planar processing
    let mut audio_planar = vec![
        audio_buffer_left.as_mut_slice(),
        audio_buffer_right.as_mut_slice(),
    ];

    // Test planar audio processing
    match processor.process_planar(&mut audio_planar) {
        Ok(()) => println!("Planar processing succeeded"),
        Err(e) => println!("Planar processing failed: {}", e),
    }

    // Create interleaved test audio (all channels mixed together)
    let mut audio_buffer_interleaved = vec![0.0f32; config.num_channels * config.num_frames];

    // Test interleaved audio processing
    match processor.process_interleaved(&mut audio_buffer_interleaved) {
        Ok(()) => println!("Interleaved processing succeeded"),
        Err(e) => println!("Interleaved processing failed: {}", e),
    }

    // Test reset functionality
    match processor.reset() {
        Ok(()) => println!("Processor reset succeeded"),
        Err(e) => println!("Processor reset failed: {}", e),
    }

    // Voice Activity Detection
    let vad = processor.create_vad();
    vad.set_parameter(VadParameter::SpeechHoldDuration, 0.08)?;
    vad.set_parameter(VadParameter::Sensitivity, 7.0)?;

    let speech_hold_duration = vad.parameter(VadParameter::SpeechHoldDuration)?;
    println!("Speech hold duration: {}", speech_hold_duration);

    let sensitivity = vad.parameter(VadParameter::Sensitivity)?;
    println!("Sensitivity: {}", sensitivity);

    if vad.is_speech_detected() {
        println!("VAD detected speech");
    } else {
        println!("VAD did not detect speech");
    }

    // Clean up is handled automatically by Rust's Drop trait
    println!("All tests completed");

    Ok(())
}
