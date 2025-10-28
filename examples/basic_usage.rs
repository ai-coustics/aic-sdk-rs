use aic_sdk::{Model, ModelType, Parameter};
use std::env;

const NUM_CHANNELS: u16 = 2;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Display library version
    println!("ai-coustics SDK version: {}", aic_sdk::get_version());

    // Get license key from environment variable
    let license = env::var("AIC_SDK_LICENSE").map_err(|_| {
        eprintln!("Error: AIC_SDK_LICENSE environment variable not set");
        eprintln!("Please set it with: export AIC_SDK_LICENSE=your_license_key");
        std::io::Error::new(std::io::ErrorKind::NotFound, "AIC_SDK_LICENSE not set")
    })?;

    // Model creation with license key
    let mut model = Model::new(ModelType::QuailS48, &license)?;
    println!("Model created successfully");

    // Get optimal settings
    let optimal_sample_rate = model.optimal_sample_rate()?;
    println!("Optimal sample rate: {} Hz", optimal_sample_rate);

    let optimal_num_frames = model.optimal_num_frames(optimal_sample_rate)?;
    println!("Optimal frame count: {}", optimal_num_frames);

    // Initialize with basic audio config
    model.initialize(optimal_sample_rate, NUM_CHANNELS, optimal_num_frames, true)?;
    println!("Model initialized successfully");

    // Get output delay
    let delay = model.output_delay()?;
    println!("Output delay: {} samples", delay);

    // Test parameter setting and getting
    model.set_parameter(Parameter::EnhancementLevel, 0.7)?;
    println!("Parameter set successfully");

    let enhancement_level = model.get_parameter(Parameter::EnhancementLevel)?;
    println!("Enhancement level: {}", enhancement_level);

    // Create minimal test audio - planar format (separate buffers for each channel)
    let mut audio_buffer_left = vec![0.0f32; optimal_num_frames];
    let mut audio_buffer_right = vec![0.0f32; optimal_num_frames];

    // Create mutable references for planar processing
    let mut audio_planar = vec![
        audio_buffer_left.as_mut_slice(),
        audio_buffer_right.as_mut_slice(),
    ];

    // Test planar audio processing
    match model.process_planar(&mut audio_planar) {
        Ok(()) => println!("Planar processing succeeded"),
        Err(e) => println!("Planar processing failed: {}", e),
    }

    // Create interleaved test audio (all channels mixed together)
    let mut audio_buffer_interleaved = vec![0.0f32; NUM_CHANNELS as usize * optimal_num_frames];

    // Test interleaved audio processing
    match model.process_interleaved(
        &mut audio_buffer_interleaved,
        NUM_CHANNELS,
        optimal_num_frames,
    ) {
        Ok(()) => println!("Interleaved processing succeeded"),
        Err(e) => println!("Interleaved processing failed: {}", e),
    }

    // Test reset functionality
    match model.reset() {
        Ok(()) => println!("Model reset succeeded"),
        Err(e) => println!("Model reset failed: {}", e),
    }

    // Clean up is handled automatically by Rust's Drop trait
    println!("All tests completed");

    Ok(())
}
