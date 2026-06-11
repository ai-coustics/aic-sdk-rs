use aic_sdk::{FileAnalyzer, Model};
use std::{
    env,
    io::{Error, ErrorKind},
    path::Path,
};

const MODEL: &str = "tyto-l-16khz";
const STEP_SECONDS: usize = 5;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let audio_path = env::args().nth(1).ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidInput,
            "usage: cargo run --example analyze_file --features download-model -- <audio-file>",
        )
    })?;

    let license = env::var("AIC_SDK_LICENSE").expect("AIC_SDK_LICENSE not set");

    let audio = load_mono_audio(&audio_path)?;

    let model_path = Model::download(MODEL, "target")?;
    let model = Model::from_file(&model_path)?;
    let mut analyzer = FileAnalyzer::new(&model, &license)?;

    println!("Model loaded from {}", model_path.display());
    println!(
        "Analyzing {} at {} Hz, {} mono sample(s), {} second step",
        audio_path,
        audio.sample_rate,
        audio.samples.len(),
        STEP_SECONDS,
    );

    let step_samples = audio.sample_rate as usize * STEP_SECONDS;

    let results = analyzer.analyze(&audio.samples, audio.sample_rate, Some(step_samples))?;

    println!();
    println!(" time | risk  | reverb | loud  | intf  | media | noise | loss");
    println!("------+-------+--------+-------+-------+-------+-------+------");
    for (index, result) in results.iter().enumerate() {
        println!(
            "{:>4}s | {:>5.3} | {:>6.3} | {:>5.3} | {:>5.3} | {:>5.3} | {:>5.3} | {:>4.3}",
            index * STEP_SECONDS,
            result.risk_score,
            result.speaker_reverb,
            result.speaker_loudness,
            result.interfering_speech,
            result.media_speech,
            result.noise,
            result.packet_loss,
        );
    }

    Ok(())
}

struct MonoAudio {
    sample_rate: u32,
    samples: Vec<f32>,
}

fn load_mono_audio(path: impl AsRef<Path>) -> Result<MonoAudio, Box<dyn std::error::Error>> {
    let audio: audio_file::Audio<f32> = audio_file::read(path, audio_file::ReadConfig::default())?;
    let num_channels = audio.num_channels as usize;

    if num_channels == 0 {
        return Err(Error::new(ErrorKind::InvalidData, "audio file has no channels").into());
    }

    if !audio.samples_interleaved.len().is_multiple_of(num_channels) {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "interleaved sample count is not divisible by channel count",
        )
        .into());
    }

    let samples = if num_channels == 1 {
        audio.samples_interleaved
    } else {
        let scale = 1.0 / num_channels as f32;
        audio
            .samples_interleaved
            .chunks_exact(num_channels)
            .map(|frame| frame.iter().sum::<f32>() * scale)
            .collect()
    };

    Ok(MonoAudio {
        sample_rate: audio.sample_rate,
        samples,
    })
}
