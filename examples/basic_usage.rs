use aic_sdk::{Model, Processor, ProcessorConfig, ProcessorParameter};
use std::env;

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
    let model_path = Model::download("quail-vf-2.1-s-16khz", "target")?;
    let model = Model::from_file(&model_path)?;
    println!("Model loaded from {}", model_path.display());

    // Get optimal ProcessorConfig from Model
    let config = ProcessorConfig::optimal(&model).with_variable_block_size(true);

    // Create processor with license key
    let mut processor = Processor::new(&model, &license)?.with_config(&config)?;
    println!(
        "Processor created and initialized successfully with: Sample rate: {} Hz, Block size: {}",
        config.sample_rate, config.block_size
    );

    // Process mono audio
    let mut audio = vec![0.0; config.block_size];
    processor.process(&mut audio)?;

    // Get processor context for thread safe interaction with parameters
    let proc_ctx = processor.context();

    // Get the delay applied to the audio
    let delay = proc_ctx.audio_delay();
    println!("Audio delay: {} samples", delay);

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

    // Exercise the bearer-token refresh path. The license used here is not necessarily a JWT,
    // so an error is acceptable. This call exists mainly to cover the FFI signature (relevant
    // for the hand-maintained runtime-linking symbol table).
    match proc_ctx.update_bearer_token(&license) {
        Ok(()) => println!("Bearer token updated"),
        Err(e) => println!(
            "Bearer token update returned (expected for non-JWT keys): {}",
            e
        ),
    }

    // Voice activity detection lives in a separate `Vad` instance, see `examples/vad.rs`.

    // End the telemetry session on demand instead of waiting for the processor to be dropped.
    // The processor can no longer process audio after this call.
    processor.terminate_session()?;
    println!("Telemetry session terminated");

    // Clean up is handled automatically by Rust's Drop trait
    println!("All tests completed");

    Ok(())
}
