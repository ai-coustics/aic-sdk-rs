use crate::{AicError, Model, Processor, ProcessorConfig, ProcessorContext, VadContext};
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

static RAYON_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

fn pool() -> &'static rayon::ThreadPool {
    RAYON_POOL.get_or_init(|| {
        let num_threads = std::env::var("AIC_NUM_THREADS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|&n| n > 0)
            .unwrap_or_else(|| {
                std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(1)
            });

        rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .thread_name(|i| format!("aic-processing-thread-{i}"))
            .build()
            .expect("failed to build aic thread-pool")
    })
}

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
    processor_context: Arc<ProcessorContext>,
    vad_context: Arc<VadContext>,
}

impl ProcessorAsync {
    /// Creates a new async audio enhancement processor instance.
    ///
    /// See [`Processor::new`] for details.
    pub fn new(model: &Model<'static>, license_key: &str) -> Result<Self, AicError> {
        let processor = Processor::new(model, license_key)?;
        let processor_context = Arc::new(processor.processor_context());
        let vad_context = Arc::new(processor.vad_context());
        Ok(Self {
            inner: Arc::new(Mutex::new(processor)),
            processor_context,
            vad_context,
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
        let mut processor = inner.lock_owned().await;
        tokio::task::spawn_blocking(move || processor.initialize(&config))
            .await
            .expect("spawn_blocking task panicked")
    }

    /// Processes audio with interleaved channel data.
    ///
    /// This is a convenience wrapper around
    /// [`ProcessorAsync::process_interleaved_owned`]. It copies `audio` into an
    /// owned buffer before processing and copies the processed samples back before
    /// returning.
    ///
    /// See [`Processor::process_interleaved`] for details on the memory layout.
    pub async fn process_interleaved(&self, audio: &mut [f32]) -> Result<(), AicError> {
        let buf = self.process_interleaved_owned(audio.to_vec()).await?;
        audio.copy_from_slice(&buf);
        Ok(())
    }

    /// Processes owned audio with interleaved channel data.
    ///
    /// This method takes ownership of `audio`, moves it to a background processing
    /// thread, and returns the processed buffer. Use this when the caller can hand
    /// over ownership and wants to avoid the extra copy-back performed by
    /// [`ProcessorAsync::process_interleaved`].
    ///
    /// See [`Processor::process_interleaved`] for details on the memory layout.
    pub async fn process_interleaved_owned(
        &self,
        mut audio: Vec<f32>,
    ) -> Result<Vec<f32>, AicError> {
        let inner = Arc::clone(&self.inner);
        let (tx, rx) = tokio::sync::oneshot::channel();
        let mut processor = inner.lock_owned().await;
        pool().spawn(move || {
            let result = processor.process_interleaved(&mut audio).map(|_| audio);
            let _ = tx.send(result);
        });
        rx.await.expect("Rayon worker dropped")
    }

    /// Processes audio with separate buffers for each channel (planar layout).
    ///
    /// This is a convenience wrapper around [`ProcessorAsync::process_planar_owned`].
    /// It copies each channel into an owned buffer before processing and copies the
    /// processed samples back before returning.
    ///
    /// See [`Processor::process_planar`] for details on the memory layout.
    pub async fn process_planar<V: AsMut<[f32]> + AsRef<[f32]>>(
        &self,
        audio: &mut [V],
    ) -> Result<(), AicError> {
        let buf = audio.iter().map(|ch| ch.as_ref().to_vec()).collect();
        let buf = self.process_planar_owned(buf).await?;
        for (dst, src) in audio.iter_mut().zip(buf.iter()) {
            dst.as_mut().copy_from_slice(src);
        }
        Ok(())
    }

    /// Processes owned audio with separate buffers for each channel (planar layout).
    ///
    /// This method takes ownership of `audio`, moves it to a background processing
    /// thread, and returns the processed channel buffers. Use this when the caller
    /// can hand over ownership and wants to avoid the extra copy-back performed by
    /// [`ProcessorAsync::process_planar`].
    ///
    /// See [`Processor::process_planar`] for details on the memory layout.
    pub async fn process_planar_owned(
        &self,
        mut audio: Vec<Vec<f32>>,
    ) -> Result<Vec<Vec<f32>>, AicError> {
        let inner = Arc::clone(&self.inner);
        let (tx, rx) = tokio::sync::oneshot::channel();
        let mut processor = inner.lock_owned().await;
        pool().spawn(move || {
            let result = processor.process_planar(&mut audio).map(|_| audio);
            let _ = tx.send(result);
        });
        rx.await.expect("Rayon worker dropped")
    }

    /// Processes audio with sequential channel data.
    ///
    /// This is a convenience wrapper around
    /// [`ProcessorAsync::process_sequential_owned`]. It copies `audio` into an
    /// owned buffer before processing and copies the processed samples back before
    /// returning.
    ///
    /// See [`Processor::process_sequential`] for details on the memory layout.
    pub async fn process_sequential(&self, audio: &mut [f32]) -> Result<(), AicError> {
        let buf = self.process_sequential_owned(audio.to_vec()).await?;
        audio.copy_from_slice(&buf);
        Ok(())
    }

    /// Processes owned audio with sequential channel data.
    ///
    /// This method takes ownership of `audio`, moves it to a background processing
    /// thread, and returns the processed buffer. Use this when the caller can hand
    /// over ownership and wants to avoid the extra copy-back performed by
    /// [`ProcessorAsync::process_sequential`].
    ///
    /// See [`Processor::process_sequential`] for details on the memory layout.
    pub async fn process_sequential_owned(
        &self,
        mut audio: Vec<f32>,
    ) -> Result<Vec<f32>, AicError> {
        let inner = Arc::clone(&self.inner);
        let (tx, rx) = tokio::sync::oneshot::channel();
        let mut processor = inner.lock_owned().await;
        pool().spawn(move || {
            let result = processor.process_sequential(&mut audio).map(|_| audio);
            let _ = tx.send(result);
        });
        rx.await.expect("Rayon worker dropped")
    }

    /// Returns a [`ProcessorContext`] for real-time parameter control.
    ///
    /// The handle is created once at construction time and shared across calls;
    /// every caller gets a clone of the same underlying context.
    ///
    /// See [`Processor::processor_context`] for details.
    pub fn processor_context(&self) -> Arc<ProcessorContext> {
        Arc::clone(&self.processor_context)
    }

    /// Returns a [`VadContext`] for voice activity detection.
    ///
    /// The handle is created once at construction time and shared across calls;
    /// every caller gets a clone of the same underlying context.
    ///
    /// See [`Processor::vad_context`] for details.
    pub fn vad_context(&self) -> Arc<VadContext> {
        Arc::clone(&self.vad_context)
    }
}
