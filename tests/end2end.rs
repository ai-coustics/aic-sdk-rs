use std::path::Path;

use aic_sdk::{Model, Processor, ProcessorConfig, ProcessorParameter, Vad};

// Shared with the unit tests in `src/`, so both resolve models through the same lock.
#[path = "../src/test_support.rs"]
mod test_support;
use test_support::{license_key, test_model_path};

pub const TEST_AUDIO_PATH: &str = "tests/data/test_signal.wav";
pub const TEST_AUDIO_ENHANCED_PATH: &str = "tests/data/test_signal_enhanced.wav";
pub const VAD_RESULTS_PATH: &str = "tests/data/vad_results.json";

/// Enhancement model used for the audio enhancement tests.
const ENHANCEMENT_MODEL_ID: &str = "quail-vf-2.2-s-16khz";
/// Dedicated VAD model used for the voice activity detection tests. Enhancement models cannot
/// be used for voice activity detection since the SDK dropped energy-based VADs.
const VAD_MODEL_ID: &str = "vad-2.1-xxs-16khz";

fn load_audio(path: impl AsRef<Path>) -> audio_file::Audio<f32> {
    audio_file::read(path, audio_file::ReadConfig::default()).expect("Failed to read audio file")
}

/// Tests audio enhancement by processing an entire mono file containing voice in a single pass.
/// Uses a non-optimal block size (full file length) to verify the internal block adapter handles
/// arbitrary input sizes correctly. Uses a reduced enhancement level (0.9) to exercise
/// non-default parameter paths. Compares output against a pre-generated reference file.
#[test]
fn process_full_file() {
    let audio = load_audio(TEST_AUDIO_PATH);
    let model =
        Model::from_file(test_model_path(ENHANCEMENT_MODEL_ID)).expect("Failed to load model");

    let config = ProcessorConfig {
        sample_rate: audio.sample_rate,
        block_size: audio.samples_interleaved.len(),
        variable_block_size: false,
    };

    let mut processor = Processor::new(&model, &license_key())
        .expect("Failed to create processor")
        .with_config(&config)
        .expect("Failed to initialize processor");

    let proc_ctx = processor.context();
    proc_ctx
        .set_parameter(ProcessorParameter::EnhancementLevel, 0.9)
        .expect("Failed to set enhancement level");

    let mut samples = audio.samples_interleaved;
    processor
        .process(&mut samples)
        .expect("Failed to process audio");

    let expected = load_audio(TEST_AUDIO_ENHANCED_PATH);
    for (&sample, expected) in samples.iter().zip(expected.samples_interleaved) {
        assert!(approx::abs_diff_eq!(sample, expected, epsilon = 1e-6));
    }
}

/// Runs the test signal through a VAD model in optimal-sized blocks and returns one speech
/// detection result per block.
fn speech_detection_per_block() -> Vec<bool> {
    let audio = load_audio(TEST_AUDIO_PATH);
    let model = Model::from_file(test_model_path(VAD_MODEL_ID)).expect("Failed to load model");

    let config = ProcessorConfig {
        sample_rate: audio.sample_rate,
        block_size: model.optimal_block_size(audio.sample_rate),
        variable_block_size: false,
    };

    let mut vad = Vad::new(&model, &license_key())
        .expect("Failed to create VAD")
        .with_config(&config)
        .expect("Failed to initialize VAD");

    let vad_ctx = vad.context();

    let samples = audio.samples_interleaved;
    let block_size = config.block_size;
    let mut speech_detected_results = Vec::new();

    for chunk in samples.chunks(block_size) {
        if chunk.len() == block_size {
            vad.process(chunk).expect("Failed to process block");
            speech_detected_results.push(vad_ctx.is_speech_detected());
        }
    }

    speech_detected_results
}

/// Tests block-based voice activity detection.
/// Processes audio in optimal-sized blocks and collects per-block speech detection results,
/// then compares the sequence against a pre-generated reference to ensure deterministic
/// behavior.
#[test]
fn process_blocks_with_vad() {
    let speech_detected_results = speech_detection_per_block();

    let expected_json =
        std::fs::read_to_string(VAD_RESULTS_PATH).expect("Failed to read VAD results");
    let expected_results: Vec<bool> =
        serde_json::from_str(&expected_json).expect("Failed to parse VAD results");
    assert_eq!(speech_detected_results, expected_results);
}

/// Tests that resetting the VAD state clears the published prediction immediately, so query
/// APIs do not return stale values from the previous stream.
#[test]
fn vad_reset_clears_published_prediction() {
    let audio = load_audio(TEST_AUDIO_PATH);
    let model = Model::from_file(test_model_path(VAD_MODEL_ID)).expect("Failed to load model");

    let config = ProcessorConfig {
        sample_rate: audio.sample_rate,
        block_size: model.optimal_block_size(audio.sample_rate),
        variable_block_size: false,
    };

    let mut vad = Vad::new(&model, &license_key())
        .expect("Failed to create VAD")
        .with_config(&config)
        .expect("Failed to initialize VAD");

    let vad_ctx = vad.context();

    let samples = audio.samples_interleaved;
    let block_size = config.block_size;

    let mut speech_was_detected = false;
    for chunk in samples.chunks(block_size) {
        if chunk.len() == block_size {
            vad.process(chunk).expect("Failed to process block");
            if vad_ctx.is_speech_detected() {
                speech_was_detected = true;
                break;
            }
        }
    }
    assert!(
        speech_was_detected,
        "the test signal contains speech, so the VAD should detect it"
    );

    vad_ctx.reset().expect("Failed to reset VAD state");

    assert!(!vad_ctx.is_speech_detected());
    assert_eq!(vad_ctx.raw_vad_probability(), 0.0);
}
