use aic_sdk::{Model, ProcessorConfig, Vad, VadParameter};
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Display library version
    println!("ai-coustics SDK version: {}", aic_sdk::get_sdk_version());

    // Get license key from environment variable
    let license = env::var("AIC_SDK_LICENSE").expect("AIC_SDK_LICENSE environment variable");

    // Voice activity detection requires a dedicated VAD model. Enhancement models are
    // rejected with `AicError::ModelTypeUnsupported`.
    // Select a model id at https://artifacts.ai-coustics.io/
    let model_path = Model::download("vad-2.1-xxs-16khz", "target")?;
    let model = Model::from_file(&model_path)?;
    println!("Model loaded from {}", model_path.display());

    // Get optimal ProcessorConfig from Model
    let config = ProcessorConfig::optimal(&model);

    // Create the VAD with the license key and initialize it
    let mut vad = Vad::new(&model, &license)?.with_config(&config)?;
    println!(
        "VAD created and initialized successfully with: Sample rate: {} Hz, Block size: {}",
        config.sample_rate, config.block_size
    );

    // Get VAD context for thread safe interaction with the prediction and its parameters
    let vad_ctx = vad.context();

    // How far the prediction lags behind the input
    println!("Prediction delay: {} samples", vad_ctx.output_delay());

    // Configure the detector. Sensitivity is the probability threshold of the model output.
    vad_ctx.set_parameter(VadParameter::SpeechHoldDuration, 0.08)?;
    vad_ctx.set_parameter(VadParameter::Sensitivity, 0.5)?;
    vad_ctx.set_parameter(VadParameter::MinimumSpeechDuration, 0.0)?;

    let speech_hold_duration = vad_ctx.parameter(VadParameter::SpeechHoldDuration)?;
    println!("Speech hold duration: {}", speech_hold_duration);

    let sensitivity = vad_ctx.parameter(VadParameter::Sensitivity)?;
    println!("Sensitivity: {}", sensitivity);

    // Feed mono audio to the detector. The audio block is not modified, it only updates the
    // prediction. Replace the silence below with your own audio.
    let mut audio = vec![0.0; config.block_size];
    vad.process(&mut audio)?;

    if vad_ctx.is_speech_detected() {
        println!("VAD detected speech");
    } else {
        println!("VAD did not detect speech");
    }
    println!("Raw probability: {}", vad_ctx.raw_vad_probability());

    // Clear the prediction and all internal state, e.g. when the stream is interrupted
    vad_ctx.reset()?;

    // Exercise the bearer-token refresh path. The license used here is not necessarily a JWT,
    // so an error is acceptable. This call exists mainly to cover the FFI signature (relevant
    // for the generated runtime-linking symbol table).
    match vad_ctx.update_bearer_token(&license) {
        Ok(()) => println!("Bearer token updated"),
        Err(e) => println!(
            "Bearer token update returned (expected for non-JWT keys): {}",
            e
        ),
    }

    // End the telemetry session on demand instead of waiting for the VAD to be dropped.
    // The VAD can no longer process audio after this call.
    vad.terminate_session()?;
    println!("Telemetry session terminated");

    Ok(())
}
