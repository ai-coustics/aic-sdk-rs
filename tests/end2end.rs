use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use aic_sdk::{Model, Processor, ProcessorConfig, ProcessorParameter};

pub const TEST_AUDIO_PATH: &str = "tests/data/test_signal.wav";
pub const TEST_AUDIO_ENHANCED_PATH: &str = "tests/data/test_signal_enhanced.wav";
pub const VAD_RESULTS_PATH: &str = "tests/data/vad_results.json";

fn download_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn find_existing_model(target_dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(target_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "aicmodel")
            && path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("sparrow_xxs_48khz"))
        {
            return Some(path);
        }
    }
    None
}

/// Downloads the test model `sparrow-xxs-48khz` into the crate's `target/` directory.
/// Returns the path to the downloaded model file.
fn get_test_model_path() -> PathBuf {
    let target_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target");

    if let Some(existing) = find_existing_model(&target_dir) {
        return existing;
    }

    let _guard = download_lock().lock().unwrap();
    if let Some(existing) = find_existing_model(&target_dir) {
        return existing;
    }

    Model::download("sparrow-xxs-48khz", &target_dir).expect("Failed to download test model")
}

fn license_key() -> String {
    std::env::var("AIC_SDK_LICENSE").expect("AIC_SDK_LICENSE environment variable not set")
}

struct TestAudio {
    sample_rate: u32,
    num_channels: usize,
    num_frames: usize,
    interleaved_samples: Vec<f32>,
}

fn load_wav_audio(path: impl AsRef<Path>) -> TestAudio {
    let reader = hound::WavReader::open(path).expect("Failed to open WAV file");
    let spec = reader.spec();
    let num_channels = spec.channels as usize;
    let sample_rate = spec.sample_rate;

    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .map(|s| s.expect("Failed to read sample"))
            .collect(),
        hound::SampleFormat::Int => {
            let bits = spec.bits_per_sample;
            let max_value = (1 << (bits - 1)) as f32;
            reader
                .into_samples::<i32>()
                .map(|s| s.expect("Failed to read sample") as f32 / max_value)
                .collect()
        }
    };

    let num_frames = samples.len() / num_channels;

    TestAudio {
        sample_rate,
        num_channels,
        num_frames,
        interleaved_samples: samples,
    }
}

fn interleaved_to_sequential(interleaved: &[f32], num_channels: usize) -> Vec<f32> {
    let num_frames = interleaved.len() / num_channels;
    let mut sequential = vec![0.0f32; interleaved.len()];
    for frame in 0..num_frames {
        for ch in 0..num_channels {
            sequential[ch * num_frames + frame] = interleaved[frame * num_channels + ch];
        }
    }
    sequential
}

fn sequential_to_interleaved(sequential: &[f32], num_channels: usize) -> Vec<f32> {
    let num_frames = sequential.len() / num_channels;
    let mut interleaved = vec![0.0f32; sequential.len()];
    for frame in 0..num_frames {
        for ch in 0..num_channels {
            interleaved[frame * num_channels + ch] = sequential[ch * num_frames + frame];
        }
    }
    interleaved
}

fn interleaved_to_planar(interleaved: &[f32], num_channels: usize) -> Vec<Vec<f32>> {
    let num_frames = interleaved.len() / num_channels;
    let mut planar = vec![vec![0.0f32; num_frames]; num_channels];
    for frame in 0..num_frames {
        for ch in 0..num_channels {
            planar[ch][frame] = interleaved[frame * num_channels + ch];
        }
    }
    planar
}

fn planar_to_interleaved(planar: &[Vec<f32>]) -> Vec<f32> {
    let num_channels = planar.len();
    let num_frames = planar[0].len();
    let mut interleaved = vec![0.0f32; num_channels * num_frames];
    for frame in 0..num_frames {
        for ch in 0..num_channels {
            interleaved[frame * num_channels + ch] = planar[ch][frame];
        }
    }
    interleaved
}

/// Tests audio enhancement by processing an entire stereo file containing voice in a single pass.
/// Uses a non-optimal frame size (full file length) to verify the internal frame adapter handles
/// arbitrary input sizes correctly. Uses a reduced enhancement level (0.9) and slightly lower
/// voice gain (0.9) to exercise non-default parameter paths. Compares output against a
/// pre-generated reference file.
#[test]
fn process_full_file_interleaved() {
    let audio = load_wav_audio(TEST_AUDIO_PATH);
    let model = Model::from_file(get_test_model_path()).expect("Failed to load model");

    let config = ProcessorConfig {
        sample_rate: audio.sample_rate,
        num_channels: audio.num_channels as u16,
        num_frames: audio.num_frames,
        allow_variable_frames: false,
    };

    let mut processor = Processor::new(&model, &license_key())
        .expect("Failed to create processor")
        .with_config(&config)
        .expect("Failed to initialize processor");

    let proc_ctx = processor.processor_context();
    proc_ctx
        .set_parameter(ProcessorParameter::EnhancementLevel, 0.9)
        .expect("Failed to set enhancement level");
    proc_ctx
        .set_parameter(ProcessorParameter::VoiceGain, 0.9)
        .expect("Failed to set voice gain");

    let mut samples = audio.interleaved_samples.clone();
    processor
        .process_interleaved(&mut samples)
        .expect("Failed to process audio");

    let expected_output = load_wav_audio(TEST_AUDIO_ENHANCED_PATH);

    for (&sample1, sample2) in samples.iter().zip(expected_output.interleaved_samples) {
        assert!(approx::abs_diff_eq!(sample1, sample2, epsilon = 1e-6));
    }
}

/// Tests audio enhancement using sequential sample layout.
/// Converts the test audio to sequential format (all samples for channel 0, then channel 1, etc.),
/// processes it, and verifies the output matches the reference after converting back to interleaved.
#[test]
fn process_full_file_sequential() {
    let audio = load_wav_audio(TEST_AUDIO_PATH);
    let model = Model::from_file(get_test_model_path()).expect("Failed to load model");

    let config = ProcessorConfig {
        sample_rate: audio.sample_rate,
        num_channels: audio.num_channels as u16,
        num_frames: audio.num_frames,
        allow_variable_frames: false,
    };

    let mut processor = Processor::new(&model, &license_key())
        .expect("Failed to create processor")
        .with_config(&config)
        .expect("Failed to initialize processor");

    let proc_ctx = processor.processor_context();
    proc_ctx
        .set_parameter(ProcessorParameter::EnhancementLevel, 0.9)
        .expect("Failed to set enhancement level");
    proc_ctx
        .set_parameter(ProcessorParameter::VoiceGain, 0.9)
        .expect("Failed to set voice gain");

    let mut samples = interleaved_to_sequential(&audio.interleaved_samples, audio.num_channels);
    processor
        .process_sequential(&mut samples)
        .expect("Failed to process audio");

    let result = sequential_to_interleaved(&samples, audio.num_channels);
    let expected_output = load_wav_audio(TEST_AUDIO_ENHANCED_PATH);

    for (&sample1, sample2) in result.iter().zip(expected_output.interleaved_samples) {
        assert!(approx::abs_diff_eq!(sample1, sample2, epsilon = 1e-6));
    }
}

/// Tests audio enhancement using planar sample layout.
/// Converts the test audio to planar format (separate buffer per channel),
/// processes it, and verifies the output matches the reference after converting back to interleaved.
#[test]
fn process_full_file_planar() {
    let audio = load_wav_audio(TEST_AUDIO_PATH);
    let model = Model::from_file(get_test_model_path()).expect("Failed to load model");

    let config = ProcessorConfig {
        sample_rate: audio.sample_rate,
        num_channels: audio.num_channels as u16,
        num_frames: audio.num_frames,
        allow_variable_frames: false,
    };

    let mut processor = Processor::new(&model, &license_key())
        .expect("Failed to create processor")
        .with_config(&config)
        .expect("Failed to initialize processor");

    let proc_ctx = processor.processor_context();
    proc_ctx
        .set_parameter(ProcessorParameter::EnhancementLevel, 0.9)
        .expect("Failed to set enhancement level");
    proc_ctx
        .set_parameter(ProcessorParameter::VoiceGain, 0.9)
        .expect("Failed to set voice gain");

    let mut planar = interleaved_to_planar(&audio.interleaved_samples, audio.num_channels);
    processor
        .process_planar(&mut planar)
        .expect("Failed to process audio");

    let result = planar_to_interleaved(&planar);
    let expected_output = load_wav_audio(TEST_AUDIO_ENHANCED_PATH);

    for (&sample1, sample2) in result.iter().zip(expected_output.interleaved_samples) {
        assert!(approx::abs_diff_eq!(sample1, sample2, epsilon = 1e-6));
    }
}

/// Tests block-based audio processing with voice activity detection (VAD).
/// Processes audio in optimal frame-sized blocks and collects per-block speech detection results.
/// The processor is set to bypass mode to verify that VAD continues to work even when audio
/// enhancement is disabled. Compares the VAD output sequence against a pre-generated reference
/// to ensure deterministic behavior.
#[test]
fn process_blocks_with_vad() {
    let audio = load_wav_audio(TEST_AUDIO_PATH);
    let model = Model::from_file(get_test_model_path()).expect("Failed to load model");

    let config = ProcessorConfig::optimal(&model).with_num_channels(audio.num_channels as u16);

    let mut processor = Processor::new(&model, &license_key())
        .expect("Failed to create processor")
        .with_config(&config)
        .expect("Failed to initialize processor");

    let proc_ctx = processor.processor_context();
    proc_ctx
        .set_parameter(ProcessorParameter::Bypass, 1.0)
        .expect("Failed to set bypass");

    let vad_ctx = processor.vad_context();

    let mut samples = audio.interleaved_samples.clone();
    let block_size = config.num_frames * audio.num_channels;
    let mut speech_detected_results = Vec::new();

    for chunk in samples.chunks_mut(block_size) {
        if chunk.len() == block_size {
            processor
                .process_interleaved(chunk)
                .expect("Failed to process block");
            speech_detected_results.push(vad_ctx.is_speech_detected());
        }
    }

    let expected_json =
        std::fs::read_to_string(VAD_RESULTS_PATH).expect("Failed to read VAD results");
    let expected_results: Vec<bool> =
        serde_json::from_str(&expected_json).expect("Failed to parse VAD results");
    assert_eq!(speech_detected_results, expected_results);
}

/// Tests that VAD output is independent of the enhancement level.
/// Uses an enhancement level of 0.5 (instead of bypass) and verifies that the VAD results
/// match the same reference as the bypass test, confirming enhancement settings do not
/// affect voice activity detection.
#[test]
fn process_blocks_with_vad_and_enhancement() {
    let audio = load_wav_audio(TEST_AUDIO_PATH);
    let model = Model::from_file(get_test_model_path()).expect("Failed to load model");

    let config = ProcessorConfig::optimal(&model).with_num_channels(audio.num_channels as u16);

    let mut processor = Processor::new(&model, &license_key())
        .expect("Failed to create processor")
        .with_config(&config)
        .expect("Failed to initialize processor");

    let proc_ctx = processor.processor_context();
    proc_ctx
        .set_parameter(ProcessorParameter::EnhancementLevel, 0.5)
        .expect("Failed to set enhancement level");

    let vad_ctx = processor.vad_context();

    let mut samples = audio.interleaved_samples.clone();
    let block_size = config.num_frames * audio.num_channels;
    let mut speech_detected_results = Vec::new();

    for chunk in samples.chunks_mut(block_size) {
        if chunk.len() == block_size {
            processor
                .process_interleaved(chunk)
                .expect("Failed to process block");
            speech_detected_results.push(vad_ctx.is_speech_detected());
        }
    }

    // Compare against the same expected results as the bypass test
    // This verifies that VAD output is independent of enhancement level
    let expected_json =
        std::fs::read_to_string(VAD_RESULTS_PATH).expect("Failed to read VAD results");
    let expected_results: Vec<bool> =
        serde_json::from_str(&expected_json).expect("Failed to parse VAD results");
    assert_eq!(speech_detected_results, expected_results);
}
