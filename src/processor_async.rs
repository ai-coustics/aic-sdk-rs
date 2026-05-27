use crate::{
    AicError, Model, OtelConfig, Processor, ProcessorConfig, ProcessorContext, VadContext,
};
use async_lock::Mutex;
use futures_channel::oneshot;
use std::sync::{Arc, OnceLock};

static RAYON_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

fn get_global_thread_pool() -> &'static rayon::ThreadPool {
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
            .expect("failed to build aic thread pool")
    })
}

/// A wrapper around [`Processor`] for use in async contexts.
///
/// # Threading
///
/// Processing runs on a background thread pool shared across all
/// [`ProcessorAsync`] instances. The pool defaults to one thread per logical
/// CPU. Override with the `AIC_NUM_THREADS` environment variable, which is
/// read once on first use.
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
///     let processor = ProcessorAsync::new(&model, &license_key)?.with_config(&config).await?;
///
///     let mut audio = vec![0.0f32; config.num_channels as usize * config.num_frames];
///     let audio = processor.process_interleaved(audio).await?;
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

    /// Creates a new async audio enhancement processor instance with explicit
    /// OpenTelemetry configuration.
    ///
    /// See [`Processor::with_otel_config`] for details.
    pub fn with_otel_config(
        model: &Model<'static>,
        license_key: &str,
        otel_config: &OtelConfig,
    ) -> Result<Self, AicError> {
        let processor = Processor::with_otel_config(model, license_key, otel_config)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(processor)),
        })
    }

    /// Initializes the async processor with the given configuration.
    ///
    /// This is a convenience method that calls [`ProcessorAsync::initialize`]
    /// internally and returns `self`.
    pub async fn with_config(self, config: &ProcessorConfig) -> Result<Self, AicError> {
        self.initialize(config).await?;
        Ok(self)
    }

    /// Initializes the processor with the given configuration.
    ///
    /// See [`Processor::initialize`] for details.
    ///
    /// # Warning
    /// This allocates memory internally. Do not call from latency-sensitive paths.
    pub async fn initialize(&self, config: &ProcessorConfig) -> Result<(), AicError> {
        let config = config.clone();
        let (tx, rx) = oneshot::channel();
        let mut processor = self.inner.lock_arc().await;
        get_global_thread_pool().spawn(move || {
            let _ = tx.send(processor.initialize(&config));
        });
        rx.await.expect("Rayon worker dropped")
    }

    /// Processes audio with interleaved channel data.
    ///
    /// This method takes ownership of `audio`, moves it to a background processing
    /// thread, and returns the processed buffer.
    ///
    /// See [`Processor::process_interleaved`] for details on the memory layout.
    pub async fn process_interleaved(&self, mut audio: Vec<f32>) -> Result<Vec<f32>, AicError> {
        let (tx, rx) = oneshot::channel();
        let mut processor = self.inner.lock_arc().await;
        get_global_thread_pool().spawn(move || {
            let result = processor.process_interleaved(&mut audio).map(|_| audio);
            let _ = tx.send(result);
        });
        rx.await.expect("Rayon worker dropped")
    }

    /// Processes audio with separate buffers for each channel (planar layout).
    ///
    /// This method takes ownership of `audio`, moves it to a background processing
    /// thread, and returns the processed channel buffers.
    ///
    /// See [`Processor::process_planar`] for details on the memory layout.
    pub async fn process_planar(
        &self,
        mut audio: Vec<Vec<f32>>,
    ) -> Result<Vec<Vec<f32>>, AicError> {
        let (tx, rx) = oneshot::channel();
        let mut processor = self.inner.lock_arc().await;
        get_global_thread_pool().spawn(move || {
            let result = processor.process_planar(&mut audio).map(|_| audio);
            let _ = tx.send(result);
        });
        rx.await.expect("Rayon worker dropped")
    }

    /// Processes audio with sequential channel data.
    ///
    /// This method takes ownership of `audio`, moves it to a background processing
    /// thread, and returns the processed buffer.
    ///
    /// See [`Processor::process_sequential`] for details on the memory layout.
    pub async fn process_sequential(&self, mut audio: Vec<f32>) -> Result<Vec<f32>, AicError> {
        let (tx, rx) = oneshot::channel();
        let mut processor = self.inner.lock_arc().await;
        get_global_thread_pool().spawn(move || {
            let result = processor.process_sequential(&mut audio).map(|_| audio);
            let _ = tx.send(result);
        });
        rx.await.expect("Rayon worker dropped")
    }

    /// Returns a [`ProcessorContext`] for real-time parameter control.
    ///
    /// See [`Processor::processor_context`] for details.
    pub async fn processor_context(&self) -> ProcessorContext {
        self.inner.lock().await.processor_context()
    }

    /// Returns a [`VadContext`] for voice activity detection.
    ///
    /// See [`Processor::vad_context`] for details.
    pub async fn vad_context(&self) -> VadContext {
        self.inner.lock().await.vad_context()
    }
}
