use crate::{AicError, Model, Processor, ProcessorConfig, ProcessorContext, VadContext};
use std::sync::Arc;
use tokio::sync::Mutex;

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
    inner: Arc<Mutex<Processor<'static>>>,
}

impl ProcessorAsync {
    /// Creates a new async audio enhancement processor instance.
    ///
    /// See [`Processor::new`] for details.
    pub fn new(model: &Model<'static>, license_key: &str) -> Result<Self, AicError> {
        let processor = Processor::new(model, license_key)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(processor)),
        })
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
        let this = Self::new(model, license_key)?;
        this.initialize(config).await?;
        Ok(this)
    }

    /// Initializes the processor with the given configuration.
    ///
    /// See [`Processor::initialize`] for details.
    ///
    /// # Warning
    /// This allocates memory internally. Do not call from latency-sensitive paths.
    pub async fn initialize(&self, config: &ProcessorConfig) -> Result<(), AicError> {
        let inner = Arc::clone(&self.inner);
        let config = config.clone();
        tokio::task::spawn_blocking(move || {
            let mut processor = inner.blocking_lock();
            processor.initialize(&config)
        })
        .await
        .expect("spawn_blocking task panicked")
    }

    /// Processes audio with interleaved channel data.
    ///
    /// See [`Processor::process_interleaved`] for details on the memory layout.
    pub async fn process_interleaved(&self, audio: &mut [f32]) -> Result<(), AicError> {
        let inner = Arc::clone(&self.inner);
        let mut buf = audio.to_vec();
        tokio::task::spawn_blocking(move || {
            let mut processor = inner.blocking_lock();
            processor.process_interleaved(&mut buf)?;
            Ok(buf)
        })
        .await
        .expect("spawn_blocking task panicked")
        .map(|buf| audio.copy_from_slice(&buf))
    }

    /// Processes audio with separate buffers for each channel (planar layout).
    ///
    /// See [`Processor::process_planar`] for details on the memory layout.
    pub async fn process_planar<V: AsMut<[f32]> + AsRef<[f32]>>(
        &self,
        audio: &mut [V],
    ) -> Result<(), AicError> {
        let inner = Arc::clone(&self.inner);
        let mut buf: Vec<Vec<f32>> = audio.iter().map(|ch| ch.as_ref().to_vec()).collect();
        tokio::task::spawn_blocking(move || {
            let mut processor = inner.blocking_lock();
            processor.process_planar(&mut buf)?;
            Ok(buf)
        })
        .await
        .expect("spawn_blocking task panicked")
        .map(|buf| {
            for (dst, src) in audio.iter_mut().zip(buf.iter()) {
                dst.as_mut().copy_from_slice(src);
            }
        })
    }

    /// Processes audio with sequential channel data.
    ///
    /// See [`Processor::process_sequential`] for details on the memory layout.
    pub async fn process_sequential(&self, audio: &mut [f32]) -> Result<(), AicError> {
        let inner = Arc::clone(&self.inner);
        let mut buf = audio.to_vec();
        tokio::task::spawn_blocking(move || {
            let mut processor = inner.blocking_lock();
            processor.process_sequential(&mut buf)?;
            Ok(buf)
        })
        .await
        .expect("spawn_blocking task panicked")
        .map(|buf| audio.copy_from_slice(&buf))
    }

    /// Creates a [`ProcessorContext`] for real-time parameter control.
    ///
    /// See [`Processor::processor_context`] for details.
    pub async fn processor_context(&self) -> ProcessorContext {
        let processor = self.inner.lock().await;
        processor.processor_context()
    }

    /// Creates a [`VadContext`] for voice activity detection.
    ///
    /// See [`Processor::vad_context`] for details.
    pub async fn vad_context(&self) -> VadContext {
        let processor = self.inner.lock().await;
        processor.vad_context()
    }
}
