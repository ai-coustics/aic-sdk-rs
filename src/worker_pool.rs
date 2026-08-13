use futures_executor::{ThreadPool, ThreadPoolBuilder};
use std::{
    panic::{self, AssertUnwindSafe},
    sync::OnceLock,
    thread,
};

/// Queues `job` on the pool shared by every [`ProcessorAsync`](crate::ProcessorAsync) and
/// [`VadAsync`](crate::VadAsync), starting its threads on first use.
///
/// The pool multiplexes every stream onto a fixed set of threads draining one shared queue, so a
/// stream is never stuck behind a busy thread while another idles, and an idle thread blocks on the
/// queue instead of looking for work to steal.
pub(crate) fn spawn(job: impl FnOnce() + Send + 'static) {
    static POOL: OnceLock<ThreadPool> = OnceLock::new();

    let pool = POOL.get_or_init(|| {
        let num_threads = std::env::var("AIC_NUM_THREADS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|&n| n > 0)
            .unwrap_or_else(|| {
                thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(1)
            });

        ThreadPoolBuilder::new()
            .pool_size(num_threads)
            .name_prefix("aic-processing-thread-")
            .create()
            .expect("failed to build aic thread pool")
    });

    // The pool never replaces a thread that unwound, so a panicking block would permanently shrink
    // it and, once every thread is gone, leave the other processors without a way to make progress.
    pool.spawn_ok(async move {
        let _ = panic::catch_unwind(AssertUnwindSafe(job));
    });
}
