#![cfg_attr(not(feature = "download-model"), allow(dead_code, unused_imports))]

#[cfg(feature = "download-model")]
use aic_sdk::{Model, Processor, ProcessorConfig, ProcessorParameter, VadParameter};
use std::env;

#[cfg(not(feature = "download-model"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err("Enable the `download-model` feature to run this example.".into())
}

#[cfg(feature = "download-model")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Display library version
    println!("ai-coustics SDK version: {}", aic_sdk::get_sdk_version());
    println!(
        "Compatible model version: {}",
        aic_sdk::get_compatible_model_version()
    );

    // Get license key from environment variable
    let license = env::var("AIC_SDK_LICENSE").expect("AIC_SDK_LICENSE environment variable");

    // Download the default model once and reuse the file
    // Select a model id at https://artifacts.ai-coustics.io/
    let model_path = Model::download("quail-xxs-48khz", "target")?;
    let model = Model::from_file(&model_path)?;
    println!("Model loaded from {}", model_path.display());

    // Create processor with license key
    let mut processor = Processor::new(&model, &license)?;
    println!("Processor created successfully");

    // Set up configuration
    let config = ProcessorConfig::optimal(&model)
        .with_num_channels(2)
        .with_allow_variable_frames(true);

    // Initialize the processor
    processor.initialize(&config)?;
    println!(
        "Processor initialized: Sample rate: {} Hz, Frames: {}, Channels: {}",
        config.sample_rate, config.num_frames, config.num_channels
    );

    // Process Audio in different data layouts (for mono audio, the layout does not matter)
    // Interleaved = [l, r, l, r, ..]
    let mut audio_interleaved = vec![0.0; config.num_channels as usize * config.num_frames];
    processor.process_interleaved(&mut audio_interleaved)?;

    // Planar = [[l, l, ..], [r, r, ..]]
    let mut audio_planar = vec![vec![0.0f32; config.num_frames]; config.num_channels as usize];
    processor.process_planar(&mut audio_planar)?;

    // Sequential = [l, l, .., r, r, ..]
    let mut audio_sequential = vec![0.0; config.num_channels as usize * config.num_frames];
    processor.process_sequential(&mut audio_sequential)?;

    // Get processor context for thread safe interaction with parameters
    let proc_ctx = processor.processor_context();

    // Get output delay
    let delay = proc_ctx.output_delay();
    println!("Output delay: {} samples", delay);

    // Test parameter setting and getting
    proc_ctx.set_parameter(ProcessorParameter::EnhancementLevel, 0.7)?;
    println!("Parameter set successfully");

    let enhancement_level = proc_ctx.parameter(ProcessorParameter::EnhancementLevel)?;
    println!("Enhancement level: {}", enhancement_level);

    // Test reset functionality
    match proc_ctx.reset() {
        Ok(()) => println!("Processor reset succeeded"),
        Err(e) => println!("Processor reset failed: {}", e),
    }

    //  Get VAD context for thread safe interaction with voice activity detection parameters
    let vad_ctx = processor.vad_context();
    vad_ctx.set_parameter(VadParameter::SpeechHoldDuration, 0.08)?;
    vad_ctx.set_parameter(VadParameter::Sensitivity, 7.0)?;

    let speech_hold_duration = vad_ctx.parameter(VadParameter::SpeechHoldDuration)?;
    println!("Speech hold duration: {}", speech_hold_duration);

    let sensitivity = vad_ctx.parameter(VadParameter::Sensitivity)?;
    println!("Sensitivity: {}", sensitivity);

    if vad_ctx.is_speech_detected() {
        println!("VAD detected speech");
    } else {
        println!("VAD did not detect speech");
    }

    // Clean up is handled automatically by Rust's Drop trait
    println!("All tests completed");

    Ok(())
}
