use crate::{AicError, Model, Processor, ProcessorConfig, ProcessorContext, VadContext};

/// An async wrapper around [`Processor`] for use in async/await contexts.
///
/// # Example
///
/// ```rust,no_run
/// use aic_sdk::{Model, ProcessorAsync, ProcessorConfig};
/// #[tokio::main]
/// async fn main() -> Result<(), aic_sdk::AicError> {
///     let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
///     let model = Model::from_file("/path/to/model.aicmodel")?;
///     let config = ProcessorConfig::optimal(&model).with_num_channels(2);
///
///     let processor = ProcessorAsync::new(&model, &license_key)?;
///     processor.initialize(&config).await?;
///
///     let mut audio = vec![0.0f32; config.num_channels as usize * config.num_frames];
///     processor.process_interleaved(&mut audio).await?;
///     Ok(())
/// }
/// ```
pub struct ProcessorAsync {
    inner: Processor<'static>,
}

impl ProcessorAsync {
    /// Creates a new async audio enhancement processor instance.
    ///
    /// See [`Processor::new`] for details.
    pub fn new(model: &Model<'static>, license_key: &str) -> Result<Self, AicError> {
        let processor = Processor::new(model, license_key)?;
        Ok(Self { inner: processor })
    }

    /// Creates a new async processor and initializes it with the given configuration.
    ///
    /// This is a convenience method combining [`ProcessorAsync::new`] and
    /// [`ProcessorAsync::initialize`].
    pub async fn with_config(
        model: &Model<'static>,
        license_key: &str,
        config: &ProcessorConfig,
    ) -> Result<Self, AicError> {
        let mut this = Self::new(model, license_key)?;
        this.initialize(config).await?;
        Ok(this)
    }

    /// Initializes the processor with the given configuration.
    ///
    /// See [`Processor::initialize`] for details.
    ///
    /// # Warning
    /// This allocates memory internally. Do not call from latency-sensitive paths.
    pub async fn initialize(&mut self, config: &ProcessorConfig) -> Result<(), AicError> {
        // SAFETY: We are using `'static` for `spawn_blocking()`,
        // but in reality `processor` is not accessible after the end of this method.
        let processor: &'static mut Processor = unsafe { std::mem::transmute(&mut self.inner) };
        // SAFETY: We are using `'static` for `spawn_blocking()`,
        // but in reality `config` is not accessible after the end of this method.
        let config: &'static _ = unsafe { std::mem::transmute(config) };
        tokio::task::spawn_blocking(move || processor.initialize(config))
            .await
            .expect("spawn_blocking task panicked")
    }

    /// Processes audio with interleaved channel data.
    ///
    /// See [`Processor::process_interleaved`] for details on the memory layout.
    pub async fn process_interleaved(&mut self, audio: &mut [f32]) -> Result<(), AicError> {
        // SAFETY: We are using `'static` for `spawn_blocking()`,
        // but in reality `processor` is not accessible after the end of this method.
        let processor: &'static mut Processor = unsafe { std::mem::transmute(&mut self.inner) };
        // SAFETY: We are using `'static` for `spawn_blocking()`,
        // but in reality `audio` is not accessible after the end of this method.
        let audio: &'static mut _ = unsafe { std::mem::transmute(audio) };
        tokio::task::spawn_blocking(move || processor.process_interleaved(audio))
            .await
            .expect("spawn_blocking task panicked")
    }

    /// Processes audio with separate buffers for each channel (planar layout).
    ///
    /// See [`Processor::process_planar`] for details on the memory layout.
    pub async fn process_planar<V: AsMut<[f32]> + AsRef<[f32]> + Send + 'static>(
        &mut self,
        audio: &mut [V],
    ) -> Result<(), AicError> {
        // SAFETY: We are using `'static` for `spawn_blocking()`,
        // but in reality `processor` is not accessible after the end of this method.
        let processor: &'static mut Processor = unsafe { std::mem::transmute(&mut self.inner) };
        // SAFETY: We are using `'static` for `spawn_blocking()`,
        // but in reality `audio` is not accessible after the end of this method.
        let audio: &'static mut [V] = unsafe { std::mem::transmute(audio) };
        tokio::task::spawn_blocking(move || processor.process_planar(audio))
            .await
            .expect("spawn_blocking task panicked")
    }

    /// Processes audio with sequential channel data.
    ///
    /// See [`Processor::process_sequential`] for details on the memory layout.
    pub async fn process_sequential(&mut self, audio: &mut [f32]) -> Result<(), AicError> {
        // SAFETY: We are using `'static` for `spawn_blocking()`,
        // but in reality `processor` is not accessible after the end of this method.
        let processor: &'static mut Processor = unsafe { std::mem::transmute(&mut self.inner) };
        // SAFETY: We are using `'static` for `spawn_blocking()`,
        // but in reality `audio` is not accessible after the end of this method.
        let audio: &'static mut _ = unsafe { std::mem::transmute(audio) };
        tokio::task::spawn_blocking(move || processor.process_sequential(audio))
            .await
            .expect("spawn_blocking task panicked")
    }

    /// Creates a [`ProcessorContext`] for real-time parameter control.
    ///
    /// See [`Processor::processor_context`] for details.
    pub async fn processor_context(&self) -> ProcessorContext {
        self.inner.processor_context()
    }

    /// Creates a [`VadContext`] for voice activity detection.
    ///
    /// See [`Processor::vad_context`] for details.
    pub async fn vad_context(&self) -> VadContext {
        self.inner.vad_context()
    }
}
