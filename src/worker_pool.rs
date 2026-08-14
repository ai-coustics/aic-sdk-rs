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
        let setting = std::env::var("AIC_NUM_THREADS").ok();

        WorkerPool::start(configured_threads(setting.as_deref()))
    })
}

/// Resolves the worker count from an `AIC_NUM_THREADS` setting, falling back to one thread per
/// logical CPU. Never returns zero: a pool with no workers would queue every block forever.
fn configured_threads(setting: Option<&str>) -> usize {
    setting
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&n| n > 0)
        .unwrap_or_else(|| {
            thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// Generous enough that a loaded CI runner never trips it, short enough that a stuck pool fails
    /// the run instead of hanging it.
    const TIMEOUT: Duration = Duration::from_secs(10);

    /// The rendezvous test waits on a latch that the jobs themselves wait on, so the outer wait must
    /// outlast the inner one to report which of the two actually failed.
    const OUTER_TIMEOUT: Duration = Duration::from_secs(30);

    /// Counts completions. The pool hands back no handle and has no drain, so a test has no other
    /// way to join the jobs it submitted.
    #[derive(Default)]
    struct Latch {
        count: Mutex<usize>,
        changed: Condvar,
    }

    impl Latch {
        fn bump(&self) {
            *self.count.lock().unwrap() += 1;
            self.changed.notify_all();
        }

        /// Returns whether `target` was reached. A timeout is reported rather than waited out, so a
        /// broken pool fails the test instead of blocking the suite forever.
        fn wait_for(&self, target: usize, timeout: Duration) -> bool {
            let (count, _) = self
                .changed
                .wait_timeout_while(self.count.lock().unwrap(), timeout, |count| *count < target)
                .unwrap();

            *count >= target
        }
    }

    /// Blocks until every worker has parked. A worker still burning its spin budget picks a job off
    /// the queue on its own, so a test that wants to exercise the wakeup path has to wait for the
    /// budget to run out first.
    fn await_parked(pool: &WorkerPool, workers: usize, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if pool.shared.lock().parked == workers {
                return true;
            }
            thread::sleep(Duration::from_millis(1));
        }

        false
    }

    #[test]
    fn every_job_runs_exactly_once() {
        const PRODUCERS: usize = 4;
        const PER_PRODUCER: usize = 50;
        const TOTAL: usize = PRODUCERS * PER_PRODUCER;

        let pool = WorkerPool::start(4);
        let done = Arc::new(Latch::default());
        let runs: Arc<Vec<AtomicUsize>> =
            Arc::new((0..TOTAL).map(|_| AtomicUsize::new(0)).collect());

        thread::scope(|scope| {
            for producer in 0..PRODUCERS {
                let pool = &pool;
                let done = Arc::clone(&done);
                let runs = Arc::clone(&runs);
                scope.spawn(move || {
                    for slot in 0..PER_PRODUCER {
                        let index = producer * PER_PRODUCER + slot;
                        let done = Arc::clone(&done);
                        let runs = Arc::clone(&runs);
                        pool.spawn(move || {
                            runs[index].fetch_add(1, Ordering::Relaxed);
                            done.bump();
                        });
                    }
                });
            }
        });

        assert!(
            done.wait_for(TOTAL, TIMEOUT),
            "only {} of {TOTAL} jobs ran",
            *done.count.lock().unwrap()
        );
        for (index, runs) in runs.iter().enumerate() {
            assert_eq!(
                runs.load(Ordering::Relaxed),
                1,
                "job {index} ran the wrong number of times"
            );
        }

        // Every job ran, so every job was popped. A non-zero hint left behind here would keep the
        // workers spinning on a queue that is empty.
        assert_eq!(pool.shared.queued.load(Ordering::Acquire), 0);
    }

    #[test]
    fn worker_survives_a_panicking_job() {
        let pool = WorkerPool::start(1);
        let done = Arc::new(Latch::default());

        pool.spawn(|| panic!("intentional panic: the worker is expected to survive this"));

        let after_panic = Arc::clone(&done);
        pool.spawn(move || after_panic.bump());
        assert!(
            done.wait_for(1, TIMEOUT),
            "the sole worker did not survive a panicking job"
        );

        let later = Arc::clone(&done);
        pool.spawn(move || later.bump());
        assert!(
            done.wait_for(2, TIMEOUT),
            "the panic poisoned the queue mutex"
        );
    }

    #[test]
    fn single_worker_runs_jobs_in_submission_order() {
        const JOBS: usize = 50;

        let pool = WorkerPool::start(1);
        let done = Arc::new(Latch::default());
        let order = Arc::new(Mutex::new(Vec::new()));

        for index in 0..JOBS {
            let done = Arc::clone(&done);
            let order = Arc::clone(&order);
            pool.spawn(move || {
                order.lock().unwrap().push(index);
                done.bump();
            });
        }

        assert!(done.wait_for(JOBS, TIMEOUT), "not every job ran");
        assert_eq!(*order.lock().unwrap(), (0..JOBS).collect::<Vec<_>>());
    }

    #[test]
    fn jobs_run_on_pool_threads() {
        let pool = WorkerPool::start(2);
        let done = Arc::new(Latch::default());
        let name = Arc::new(Mutex::new(String::new()));

        let job_done = Arc::clone(&done);
        let job_name = Arc::clone(&name);
        pool.spawn(move || {
            *job_name.lock().unwrap() = thread::current().name().unwrap_or_default().to_owned();
            job_done.bump();
        });

        assert!(done.wait_for(1, TIMEOUT), "the job never ran");
        let name = name.lock().unwrap();
        assert!(
            name.starts_with("aic-processing-thread-"),
            "the job ran on `{name}`"
        );
    }

    #[test]
    fn all_workers_run_concurrently() {
        const WORKERS: usize = 4;

        let pool = WorkerPool::start(WORKERS);
        let started = Arc::new(Latch::default());
        let finished = Arc::new(Latch::default());
        let together = Arc::new(AtomicUsize::new(0));

        // Every job below blocks until all of them are running, so none of them can free up a
        // worker for the next. Each of the four therefore has to be delivered by its own
        // `notify_one`, which is what makes this a test of the `parked > 0` shortcut in `spawn`.
        assert!(
            await_parked(&pool, WORKERS, TIMEOUT),
            "the workers never parked"
        );

        for _ in 0..WORKERS {
            let started = Arc::clone(&started);
            let finished = Arc::clone(&finished);
            let together = Arc::clone(&together);
            pool.spawn(move || {
                started.bump();
                if started.wait_for(WORKERS, TIMEOUT) {
                    together.fetch_add(1, Ordering::Relaxed);
                }
                finished.bump();
            });
        }

        assert!(
            finished.wait_for(WORKERS, OUTER_TIMEOUT),
            "not every job ran"
        );
        assert_eq!(
            together.load(Ordering::Relaxed),
            WORKERS,
            "the pool never had all {WORKERS} jobs running at once"
        );
    }

    #[test]
    fn thread_count_honours_env_override() {
        assert_eq!(configured_threads(Some("4")), 4);
    }

    #[test]
    fn thread_count_never_falls_below_one() {
        for setting in [None, Some("0"), Some(""), Some("abc"), Some("-1")] {
            assert!(
                configured_threads(setting) >= 1,
                "`{setting:?}` resolved to a pool with no workers"
            );
        }
    }
}
