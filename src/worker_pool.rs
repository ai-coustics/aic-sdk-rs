use std::{
    collections::VecDeque,
    panic::{self, AssertUnwindSafe},
    sync::{
        Arc, Condvar, Mutex, MutexGuard, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

type Job = Box<dyn FnOnce() + Send + 'static>;

/// Returns the pool shared by every [`ProcessorAsync`](crate::ProcessorAsync) and
/// [`VadAsync`](crate::VadAsync), starting its threads on first use.
pub(crate) fn global() -> &'static WorkerPool {
    static POOL: OnceLock<WorkerPool> = OnceLock::new();
    POOL.get_or_init(|| {
        let num_threads = std::env::var("AIC_NUM_THREADS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|&n| n > 0)
            .unwrap_or_else(|| {
                thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(1)
            });

        WorkerPool::start(num_threads)
    })
}

/// A fixed set of worker threads draining one shared queue.
///
/// Any worker runs any block, so a stream is never stuck behind a busy thread while another idles.
/// Workers park on the queue instead of looking for work to steal: an idle worker then costs
/// nothing, which is what makes this cheaper than a work-stealing pool whenever fewer processors
/// are active than the machine has cores.
pub(crate) struct WorkerPool {
    shared: Arc<Shared>,
}

/// Ceiling on how many rounds a worker rechecks the queue before parking. A busy stream submits
/// its next block a few microseconds after the previous result is published, so covering that gap
/// keeps a saturated worker from paying a park/unpark pair per block.
///
/// The budget is adaptive rather than fixed: it doubles while the recheck keeps catching work and
/// halves when it does not, so a pool serving more streams than it has workers stops paying for a
/// spin that never pays off, and one that is mostly idle gives its cores straight back.
const MAX_SPIN_ROUNDS: u32 = 128;

struct Shared {
    state: Mutex<State>,
    ready: Condvar,
    /// Mirrors `State::queue.len()` so a spinning worker can tell whether it is worth taking the
    /// mutex. Only ever a hint: every read is confirmed under the lock.
    queued: AtomicUsize,
}

struct State {
    queue: VecDeque<Job>,
    /// Workers blocked in `Condvar::wait`. Only mutated under the mutex, so a producer that reads
    /// zero here knows every worker is either running a job or still to recheck the queue under
    /// the lock, and will therefore observe the job it just pushed without being signalled.
    parked: usize,
}

impl WorkerPool {
    fn start(num_threads: usize) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                queue: VecDeque::new(),
                parked: 0,
            }),
            ready: Condvar::new(),
            queued: AtomicUsize::new(0),
        });

        for index in 0..num_threads {
            let shared = shared.clone();
            thread::Builder::new()
                .name(format!("aic-processing-thread-{index}"))
                .spawn(move || {
                    let mut spin_rounds = MAX_SPIN_ROUNDS;
                    loop {
                        let (job, avoided_parking) = shared.next_job(spin_rounds);
                        spin_rounds = if avoided_parking {
                            (spin_rounds * 2).clamp(1, MAX_SPIN_ROUNDS)
                        } else {
                            spin_rounds / 2
                        };

                        // A panicking block must not take the worker down with it: the other
                        // processors sharing this thread would lose their way to make progress.
                        let _ = panic::catch_unwind(AssertUnwindSafe(job));
                    }
                })
                .expect("failed to spawn aic processing thread");
        }

        Self { shared }
    }

    /// Queues `job` to run on whichever worker picks it up next.
    pub(crate) fn spawn(&self, job: impl FnOnce() + Send + 'static) {
        // Boxed before taking the lock: an allocation inside the critical section would show up
        // as contention once several streams submit blocks concurrently.
        let job: Job = Box::new(job);

        let mut state = self.shared.lock();
        state.queue.push_back(job);
        self.shared
            .queued
            .store(state.queue.len(), Ordering::Release);
        let wake_worker = state.parked > 0;
        drop(state);

        // Signalling when nothing is parked would be a wasted syscall on every block.
        if wake_worker {
            self.shared.ready.notify_one();
        }
    }
}

impl Shared {
    /// Returns the next job, and whether it arrived without the worker having to park. The caller
    /// feeds that back into `spin_rounds`.
    fn next_job(&self, spin_rounds: u32) -> (Job, bool) {
        for _ in 0..spin_rounds {
            if self.queued.load(Ordering::Acquire) > 0
                && let Some(job) = self.take_queued()
            {
                return (job, true);
            }
            for _ in 0..32 {
                std::hint::spin_loop();
            }
        }

        let mut state = self.lock();
        let mut parked = false;
        loop {
            if let Some(job) = state.queue.pop_front() {
                self.queued.store(state.queue.len(), Ordering::Release);
                return (job, !parked);
            }
            state.parked += 1;
            state = self.ready.wait(state).expect("aic worker pool poisoned");
            state.parked -= 1;
            parked = true;
        }
    }

    fn take_queued(&self) -> Option<Job> {
        let mut state = self.lock();
        let job = state.queue.pop_front()?;
        self.queued.store(state.queue.len(), Ordering::Release);
        Some(job)
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().expect("aic worker pool poisoned")
    }
}
