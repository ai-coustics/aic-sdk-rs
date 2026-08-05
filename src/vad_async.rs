use crate::{
    AicError, Model, OtelConfig, ProcessorConfig, Vad, VadContext,
    processor_async::get_global_thread_pool,
};
use async_lock::Mutex;
use futures_channel::oneshot;
use std::sync::Arc;

/// A wrapper around [`Vad`] for use in async contexts.
///
/// # Threading
///
/// Processing runs on the same background thread pool as
/// [`ProcessorAsync`](crate::ProcessorAsync), shared across all instances. The pool defaults to
/// one thread per logical CPU. Override with the `AIC_NUM_THREADS` environment variable, which is
/// read once on first use.
///
/// # Example
///
/// ```rust,no_run
/// use aic_sdk::{Model, ProcessorConfig, VadAsync};
/// #[tokio::main]
/// async fn main() -> Result<(), aic_sdk::AicError> {
///     let license_key = std::env::var("AIC_SDK_LICENSE").unwrap();
///     let model = Model::from_file("/path/to/vad_model.aicmodel")?;
///     let config = ProcessorConfig::optimal(&model);
///
///     let vad = VadAsync::new(&model, &license_key)?.with_config(&config).await?;
///     let context = vad.context().await;
///
///     let mut audio = vec![0.0f32; config.block_size];
///     for _ in 0..2 {
///         // `process` hands the block back, so the same allocation can be reused.
///         audio = vad.process(audio).await?;
///         println!("Speech detected: {}", context.is_speech_detected());
///     }
///     Ok(())
/// }
/// ```
pub struct VadAsync {
    inner: Arc<Mutex<Vad<'static>>>,
}

impl VadAsync {
    /// Creates a new async voice activity detector instance.
    ///
    /// See [`Vad::new`] for details.
    pub fn new(model: &Model<'static>, license_key: &str) -> Result<Self, AicError> {
        let vad = Vad::new(model, license_key)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(vad)),
        })
    }

    /// Creates a new async voice activity detector instance with explicit
    /// OpenTelemetry configuration.
    ///
    /// See [`Vad::with_otel_config`] for details.
    pub fn with_otel_config(
        model: &Model<'static>,
        license_key: &str,
        otel_config: &OtelConfig,
    ) -> Result<Self, AicError> {
        let vad = Vad::with_otel_config(model, license_key, otel_config)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(vad)),
        })
    }

    /// Initializes the async VAD with the given configuration.
    ///
    /// This is a convenience method that calls [`VadAsync::initialize`]
    /// internally and returns `self`.
    pub async fn with_config(self, config: &ProcessorConfig) -> Result<Self, AicError> {
        self.initialize(config).await?;
        Ok(self)
    }

    /// Initializes the VAD with the given configuration.
    ///
    /// See [`Vad::initialize`] for details.
    ///
    /// # Warning
    /// This allocates memory internally. Do not call from latency-sensitive paths.
    pub async fn initialize(&self, config: &ProcessorConfig) -> Result<(), AicError> {
        let config = config.clone();
        let (tx, rx) = oneshot::channel();
        let mut vad = self.inner.lock_arc().await;
        get_global_thread_pool().spawn(move || {
            let _ = tx.send(vad.initialize(&config));
        });
        rx.await.expect("Rayon worker dropped")
    }

    /// Processes mono audio and updates the VAD prediction.
    ///
    /// This method takes ownership of `audio`, moves it to a background processing
    /// thread, and returns the audio block unmodified. Ownership is required because the
    /// background thread outlives the borrow if this future is cancelled; handing the block
    /// back lets a streaming loop reuse the same allocation for every block.
    ///
    /// See [`Vad::process`] for details.
    pub async fn process(&self, audio: Vec<f32>) -> Result<Vec<f32>, AicError> {
        let (tx, rx) = oneshot::channel();
        let mut vad = self.inner.lock_arc().await;
        get_global_thread_pool().spawn(move || {
            let result = vad.process(&audio).map(|_| audio);
            let _ = tx.send(result);
        });
        rx.await.expect("Rayon worker dropped")
    }

    /// Terminates the telemetry session associated with this VAD.
    ///
    /// See [`Vad::terminate_session`] for details.
    ///
    /// # Warning
    /// This may block until the session is terminated, so it runs on the background
    /// thread pool rather than the calling task.
    pub async fn terminate_session(&self) -> Result<(), AicError> {
        let (tx, rx) = oneshot::channel();
        let mut vad = self.inner.lock_arc().await;
        get_global_thread_pool().spawn(move || {
            let _ = tx.send(vad.terminate_session());
        });
        rx.await.expect("Rayon worker dropped")
    }

    /// Returns a [`VadContext`] to read the prediction and control parameters.
    ///
    /// See [`Vad::context`] for details.
    pub async fn context(&self) -> VadContext {
        self.inner.lock().await.context()
    }
}
