use crate::{AicError, AnalysisResult, Analyzer, Collector, Model, ProcessorConfig, analyzer_pair};

/// Analyzes complete mono audio buffers.
///
/// `FileAnalyzer` is a convenience wrapper around a [`Collector`] and [`Analyzer`] pair for
/// non-real-time analysis of audio that is already loaded in memory.
///
/// Each call to [`analyze`](Self::analyze) configures the collector for mono input with the model's
/// optimal frame size. It analyzes independent five-second windows, advancing the start of each
/// window by `step_samples`.
///
/// For streaming or multi-channel analysis, use [`analyzer_pair`] directly.
pub struct FileAnalyzer<'model, 'a> {
    model: &'model Model<'a>,
    collector: Collector,
    analyzer: Analyzer<'a>,
}

impl<'model, 'a> FileAnalyzer<'model, 'a> {
    // TODO: This should be queried from the model, but there are no APIs
    // for that available yet. `tyto-l-16khz` has a fixed window size of 5 seconds.
    const ANALYSIS_WINDOW_SECONDS: usize = 5;

    /// Creates a new file analyzer.
    ///
    /// The collector is not initialized until [`analyze`](Self::analyze) is called. This lets the
    /// same `FileAnalyzer` instance analyze mono buffers with different sample rates or step sizes.
    ///
    /// # Arguments
    ///
    /// * `model` - The loaded model instance
    /// * `license_key` - license key for the ai-coustics SDK
    ///   (generate your key at [developers.ai-coustics.com](https://developers.ai-coustics.com/))
    ///
    /// # Returns
    ///
    /// Returns a `FileAnalyzer` if the analyzer pair can be created, otherwise an [`AicError`].
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use aic_sdk::{FileAnalyzer, Model};
    /// # let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    /// # let model = Model::from_file("/path/to/model.aicmodel")?;
    /// let mut analyzer = FileAnalyzer::new(&model, &license_key)?;
    ///
    /// let sample_rate = 16_000;
    /// let audio = vec![0.0f32; 8000];
    /// let results = analyzer.analyze(&audio, sample_rate, None)?;
    /// # Ok::<(), aic_sdk::AicError>(())
    /// ```
    pub fn new(model: &'model Model<'a>, license_key: &str) -> Result<Self, AicError> {
        let (collector, analyzer) = analyzer_pair(model, license_key)?;

        Ok(Self {
            model,
            collector,
            analyzer,
        })
    }

    /// Analyzes a complete mono audio buffer.
    ///
    /// The input slice must contain mono `f32` samples at `sample_rate`. No channel mixing or
    /// resampling is performed.
    ///
    /// The analyzer evaluates five-second windows. `FileAnalyzer` buffers a window starting at
    /// sample 0, runs the analyzer once, resets the analyzer and collector, then repeats with a
    /// window starting `step_samples` later.
    ///
    /// If `audio` is shorter than or equal to five seconds, it is padded with silence and only one
    /// result is returned. For longer signals, only complete five-second windows are analyzed after
    /// the first window.
    ///
    /// # Arguments
    ///
    /// * `audio` - Mono audio samples to analyze
    /// * `sample_rate` - Sample rate of `audio` in Hz
    /// * `step_samples` - Number of samples to advance between analysis results. Defaults to
    ///   the model's window size (no overlap in analysis windows) if `None`.
    ///
    /// # Returns
    ///
    /// Returns a list of [`AnalysisResult`] values, or an [`AicError`] if initialization,
    /// buffering, or analysis fails.
    ///
    /// # Safety
    ///
    /// This function is not real-time safe. Avoid calling it from audio threads.
    pub fn analyze(
        &mut self,
        audio: &[f32],
        sample_rate: u32,
        step_samples: Option<usize>,
    ) -> Result<Vec<AnalysisResult>, AicError> {
        if sample_rate == 0 {
            return Err(AicError::AudioConfigUnsupported);
        }

        // The analysis model consumes a fixed five-second context. Convert that duration to the
        // caller's sample rate once and use it as the size of every analysis window.
        let Some(analysis_window_samples) =
            (sample_rate as usize).checked_mul(Self::ANALYSIS_WINDOW_SECONDS)
        else {
            return Err(AicError::AudioConfigUnsupported);
        };

        let step_samples = step_samples.unwrap_or(analysis_window_samples);
        if step_samples == 0 {
            return Err(AicError::AudioConfigUnsupported);
        }

        // The collector only emits fresh spectrogram frames at the model's hop size. Feeding any
        // other frame size would add buffering inside the collector and shift the analysis timing.
        let optimal_num_frames = self.model.optimal_num_frames(sample_rate);
        if optimal_num_frames == 0 {
            return Err(AicError::AudioConfigUnsupported);
        }

        let config = ProcessorConfig {
            sample_rate,
            num_channels: 1,
            // Collector/STFT output advances at the model hop size, so always feed fixed optimal
            // frames regardless of the requested analysis step.
            num_frames: optimal_num_frames,
            allow_variable_frames: false,
        };

        self.collector.initialize(&config)?;

        let window_starts =
            Self::analysis_window_starts(audio.len(), analysis_window_samples, step_samples);

        // Short files still produce one padded five-second analysis. Longer files produce one
        // result for each complete five-second window on the step grid.
        let num_results = window_starts.len();
        let mut results = Vec::with_capacity(num_results);

        for window_start in window_starts {
            // Each result must be computed from an independent five-second span. Reset clears both
            // the analyzer and collector before buffering the next window from scratch.
            self.analyzer.reset()?;

            self.buffer_analysis_window(
                audio,
                window_start,
                analysis_window_samples,
                optimal_num_frames,
            )?;

            results.push(self.analyzer.analyze_buffered()?);
        }

        Ok(results)
    }

    fn analysis_window_starts(
        audio_len: usize,
        analysis_window_samples: usize,
        step_samples: usize,
    ) -> Vec<usize> {
        if audio_len <= analysis_window_samples {
            return vec![0];
        }

        let num_complete_followup_windows = (audio_len - analysis_window_samples) / step_samples;
        (0..=num_complete_followup_windows)
            .map(|step| step * step_samples)
            .collect()
    }

    // Buffers exactly one analysis window into the collector using fixed-size model-hop frames.
    // Missing samples are zero-padded so short first windows still reach the model's full context.
    fn buffer_analysis_window(
        &mut self,
        audio: &[f32],
        start: usize,
        window_samples: usize,
        frame_samples: usize,
    ) -> Result<(), AicError> {
        let mut frame = vec![0.0; frame_samples];
        let mut buffered_samples = 0;

        while buffered_samples < window_samples {
            let Some(frame_start) = start.checked_add(buffered_samples) else {
                return Err(AicError::AudioConfigUnsupported);
            };

            let available_samples = audio.len().saturating_sub(frame_start).min(frame_samples);

            // The collector was initialized with fixed frame size, so every call below must pass
            // exactly frame_samples samples.
            if available_samples == frame_samples {
                // Fast path: the next fixed-size frame is fully available from the source audio.
                let frame_end = frame_start + frame_samples;
                self.collector
                    .buffer_interleaved(&audio[frame_start..frame_end])?;
            } else {
                // Pad short windows or non-aligned tails with silence while still feeding the
                // collector exactly one fixed-size frame.
                frame.fill(0.0);
                if available_samples > 0 {
                    let frame_end = frame_start + available_samples;
                    frame[..available_samples].copy_from_slice(&audio[frame_start..frame_end]);
                }
                self.collector.buffer_interleaved(&frame)?;
            }

            buffered_samples += frame_samples;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::{Mutex, OnceLock},
    };

    fn download_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn find_existing_model(target_dir: &Path) -> Option<PathBuf> {
        let entries = fs::read_dir(target_dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|name| name.contains("tyto_l_16khz") && name.ends_with(".aicmodel"))
                .unwrap_or(false)
                && path.is_file()
            {
                return Some(path);
            }
        }
        None
    }

    fn get_tyto_l_16khz() -> Result<PathBuf, AicError> {
        let target_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target");

        if let Some(existing) = find_existing_model(&target_dir) {
            return Ok(existing);
        }

        let _guard = download_lock().lock().unwrap();
        if let Some(existing) = find_existing_model(&target_dir) {
            return Ok(existing);
        }

        if cfg!(feature = "download-model") {
            Model::download("tyto-l-16khz", target_dir)
        } else {
            panic!(
                "Model `tyto-l-16khz` not found in {} and `download-model` feature is disabled",
                target_dir.display()
            );
        }
    }

    fn load_test_model() -> Result<(Model<'static>, String), AicError> {
        let license_key = std::env::var("AIC_SDK_LICENSE")
            .expect("AIC_SDK_LICENSE environment variable must be set for tests");

        let model_path = get_tyto_l_16khz()?;
        let model = Model::from_file(&model_path)?;

        Ok((model, license_key))
    }

    fn assert_score_range(result: &AnalysisResult) {
        assert!((0.0..=1.0).contains(&result.risk_score));
        assert!((0.0..=1.0).contains(&result.speaker_reverb));
        assert!((0.0..=1.0).contains(&result.speaker_loudness));
        assert!((0.0..=1.0).contains(&result.interfering_speech));
        assert!((0.0..=1.0).contains(&result.media_speech));
        assert!((0.0..=1.0).contains(&result.noise));
        assert!((0.0..=1.0).contains(&result.packet_loss));
    }

    fn assert_all_scores_in_range(results: &[AnalysisResult]) {
        for result in results {
            assert_score_range(result);
        }
    }

    #[test]
    fn analysis_window_starts_returns_one_padded_window_for_short_audio() {
        assert_eq!(FileAnalyzer::analysis_window_starts(0, 80_000, 1_600), [0]);
        assert_eq!(
            FileAnalyzer::analysis_window_starts(79_999, 80_000, 1_600),
            [0]
        );
        assert_eq!(
            FileAnalyzer::analysis_window_starts(80_000, 80_000, 1_600),
            [0]
        );
    }

    #[test]
    fn analysis_window_starts_advances_by_step_for_complete_followup_windows() {
        assert_eq!(
            FileAnalyzer::analysis_window_starts(83_200, 80_000, 1_600),
            [0, 1_600, 3_200]
        );
        assert_eq!(
            FileAnalyzer::analysis_window_starts(86_400, 80_000, 1_600),
            [0, 1_600, 3_200, 4_800, 6_400]
        );
    }

    #[test]
    fn analysis_window_starts_ignores_partial_followup_windows() {
        assert_eq!(
            FileAnalyzer::analysis_window_starts(81_599, 80_000, 1_600),
            [0]
        );
        assert_eq!(
            FileAnalyzer::analysis_window_starts(83_199, 80_000, 1_600),
            [0, 1_600]
        );
    }

    #[test]
    fn new_rejects_license_key_with_nul() {
        let (model, _) = load_test_model().unwrap();

        let result = FileAnalyzer::new(&model, "invalid\0license");

        assert!(matches!(result, Err(AicError::LicenseFormatInvalid)));
    }

    #[test]
    fn analyze_rejects_zero_sample_rate_or_step_size() {
        let (model, license_key) = load_test_model().unwrap();
        let mut analyzer = FileAnalyzer::new(&model, &license_key).unwrap();
        let audio = [0.0f32; 16];

        assert_eq!(
            analyzer.analyze(&audio, 0, Some(160)),
            Err(AicError::AudioConfigUnsupported)
        );
        assert_eq!(
            analyzer.analyze(&audio, 16_000, Some(0)),
            Err(AicError::AudioConfigUnsupported)
        );
    }

    #[test]
    fn analyze_short_audio_returns_single_padded_result() {
        let (model, license_key) = load_test_model().unwrap();
        let mut analyzer = FileAnalyzer::new(&model, &license_key).unwrap();
        let sample_rate = model.optimal_sample_rate();
        let step_samples = model.optimal_num_frames(sample_rate);
        let audio = vec![0.0f32; sample_rate as usize];

        let results = analyzer
            .analyze(&audio, sample_rate, Some(step_samples))
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_all_scores_in_range(&results);
    }

    #[test]
    fn analyze_exact_window_returns_single_result() {
        let (model, license_key) = load_test_model().unwrap();
        let mut analyzer = FileAnalyzer::new(&model, &license_key).unwrap();
        let sample_rate = model.optimal_sample_rate();
        let step_samples = model.optimal_num_frames(sample_rate);
        let window_samples = sample_rate as usize * FileAnalyzer::ANALYSIS_WINDOW_SECONDS;
        let audio = vec![0.0f32; window_samples];

        let results = analyzer
            .analyze(&audio, sample_rate, Some(step_samples))
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_all_scores_in_range(&results);
    }

    #[test]
    fn analyze_defaults_step_to_analysis_window_size() {
        let (model, license_key) = load_test_model().unwrap();
        let mut analyzer = FileAnalyzer::new(&model, &license_key).unwrap();
        let sample_rate = model.optimal_sample_rate();
        let window_samples = sample_rate as usize * FileAnalyzer::ANALYSIS_WINDOW_SECONDS;
        let audio = vec![0.0f32; window_samples * 2];

        let results = analyzer.analyze(&audio, sample_rate, None).unwrap();

        assert_eq!(results.len(), 2);
        assert_all_scores_in_range(&results);
    }

    #[test]
    fn analyze_long_audio_returns_one_result_per_complete_window() {
        let (model, license_key) = load_test_model().unwrap();
        let mut analyzer = FileAnalyzer::new(&model, &license_key).unwrap();
        let sample_rate = model.optimal_sample_rate();
        let step_samples = model.optimal_num_frames(sample_rate);
        let window_samples = sample_rate as usize * FileAnalyzer::ANALYSIS_WINDOW_SECONDS;
        let audio = vec![0.0f32; window_samples + 2 * step_samples];

        let results = analyzer
            .analyze(&audio, sample_rate, Some(step_samples))
            .unwrap();

        assert_eq!(results.len(), 3);
        assert_all_scores_in_range(&results);
    }

    #[test]
    fn analyze_ignores_partial_followup_window() {
        let (model, license_key) = load_test_model().unwrap();
        let mut analyzer = FileAnalyzer::new(&model, &license_key).unwrap();
        let sample_rate = model.optimal_sample_rate();
        let step_samples = model.optimal_num_frames(sample_rate);
        let window_samples = sample_rate as usize * FileAnalyzer::ANALYSIS_WINDOW_SECONDS;
        let audio = vec![0.0f32; window_samples + step_samples - 1];

        let results = analyzer
            .analyze(&audio, sample_rate, Some(step_samples))
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_all_scores_in_range(&results);
    }
}
