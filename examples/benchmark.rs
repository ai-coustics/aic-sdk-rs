use aic_sdk::{Model, ProcessorAsync, ProcessorConfig};
use std::{
    env,
    io::Write,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{
    task::JoinSet,
    time::{self, MissedTickBehavior},
};

// Specify the model to benchmark
const MODEL: &str = "quail-vf-2.0-l-16khz";

// Interval between spawning new processor sessions
const SESSION_SPAWN_INTERVAL: Duration = Duration::from_secs(1);

// Safety margin to account for system variability
// e.g. 0.3 means 30% of the period is reserved as a safety margin,
// therefore processing time cannot exceed 70% of the period
const SAFETY_MARGIN: f64 = 0.0;

#[derive(Clone)]
struct SessionReport {
    session_id: usize,
    max_execution_time: Duration,
    error: Option<String>,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("ai-coustics SDK version: {}", aic_sdk::get_sdk_version());

    let license = env::var("AIC_SDK_LICENSE").expect("AIC_SDK_LICENSE not found");

    let model_path = Model::download(MODEL, "target")?;
    let model = Arc::new(Model::from_file(&model_path)?);
    println!("Model loaded from {}\n", model_path.display());

    let config = ProcessorConfig::optimal(&model);

    let period = Duration::from_secs_f64(config.num_frames as f64 / config.sample_rate as f64);
    let safety_margin = Duration::from_secs_f64(period.as_secs_f64() * SAFETY_MARGIN);

    println!("Model: {}", model.id());
    println!("Sample rate: {} Hz", config.sample_rate);
    println!("Frames per buffer: {}", config.num_frames);
    println!("Period: {} ms", period.as_millis());
    println!("Safety margin: {} ms\n", safety_margin.as_millis());

    println!(
        "Starting benchmark: spawning a processor session every {} second(s) until a deadline is missed or a process error occurs...\n",
        SESSION_SPAWN_INTERVAL.as_secs()
    );

    let stop = Arc::new(AtomicBool::new(false));
    let mut sessions = JoinSet::new();
    let mut reports = Vec::new();
    let mut spawned_sessions = 0usize;

    spawn_session(
        &mut sessions,
        spawned_sessions,
        Arc::clone(&model),
        license.clone(),
        config.clone(),
        period,
        safety_margin,
        Arc::clone(&stop),
    );
    spawned_sessions += 1;

    print!("*");
    std::io::stdout().flush().unwrap();

    let mut spawn_ticks = time::interval(SESSION_SPAWN_INTERVAL);
    spawn_ticks.set_missed_tick_behavior(MissedTickBehavior::Skip);
    spawn_ticks.tick().await;

    let first_failed_report = loop {
        tokio::select! {
            _ = spawn_ticks.tick() => {
                spawn_session(
                    &mut sessions,
                    spawned_sessions,
                    Arc::clone(&model),
                    license.clone(),
                    config.clone(),
                    period,
                    safety_margin,
                    Arc::clone(&stop),
                );
                spawned_sessions += 1;

                print!("*");
                if spawned_sessions.is_multiple_of(50) {
                    print!("\n");
                }
                std::io::stdout().flush().unwrap();
            }
            Some(result) = sessions.join_next() => {
                let report = result?;
                if spawned_sessions.is_multiple_of(50) {
                    println!();
                } else {
                    println!();
                }

                let is_miss = report.error.is_some();
                reports.push(report);
                if is_miss {
                    stop.store(true, Ordering::Relaxed);
                    break reports.last().cloned();
                }
            }
        }
    };

    println!("Benchmark complete\n");

    while let Some(result) = sessions.join_next().await {
        reports.push(result?);
    }
    reports.sort_by_key(|report| report.session_id);

    let mut number_of_missed_deadlines = 0usize;

    println!(" ID | Max Exec Time |   RTF   | Notes");
    println!("----+---------------+---------+------");
    for report in &reports {
        let max_ms = report.max_execution_time.as_secs_f64() * 1000.0;
        let period_ms = period.as_secs_f64() * 1000.0;
        let rtf = if period_ms > 0.0 {
            max_ms / period_ms
        } else {
            0.0
        };

        let note = match report.error.as_deref() {
            Some(reason) => {
                number_of_missed_deadlines += 1;
                format!("deadline missed: {}", reason)
            }
            None => String::new(),
        };

        println!(
            "{:>3} | {:>9.3} ms  | {:>7.3} | {}",
            report.session_id, max_ms, rtf, note
        );
    }

    println!();

    let max_ok = spawned_sessions.saturating_sub(1);
    println!(
        "System can run {} instances of this model/config concurrently while meeting real-time requirements",
        max_ok
    );

    if let Some(first_failed_report) = &first_failed_report {
        println!(
            "After spawning the {}{} session, session #{} missed its deadline ({})",
            spawned_sessions,
            number_suffix(spawned_sessions),
            first_failed_report.session_id,
            first_failed_report.error.as_deref().unwrap_or("unknown")
        );

        if number_of_missed_deadlines > 1 {
            println!(
                "Other sessions also missed deadlines after session #{}",
                first_failed_report.session_id
            );
        }
    } else {
        println!("Missed deadline in session unknown (no report)");
    }

    Ok(())
}

fn spawn_session(
    sessions: &mut JoinSet<SessionReport>,
    previous_session_id: usize,
    model: Arc<Model<'static>>,
    license: String,
    config: ProcessorConfig,
    period: Duration,
    safety_margin: Duration,
    stop: Arc<AtomicBool>,
) {
    sessions.spawn(run_session(
        previous_session_id + 1,
        model,
        license,
        config,
        period,
        safety_margin,
        stop,
    ));
}

async fn run_session(
    session_id: usize,
    model: Arc<Model<'static>>,
    license: String,
    config: ProcessorConfig,
    period: Duration,
    safety_margin: Duration,
    stop: Arc<AtomicBool>,
) -> SessionReport {
    let processor = match ProcessorAsync::with_config(&model, &license, &config).await {
        Ok(processor) => processor,
        Err(err) => {
            return SessionReport {
                session_id,
                max_execution_time: Duration::ZERO,
                error: Some(format!("processor init failed: {}", err)),
            };
        }
    };

    let mut buffer = vec![0.0f32; config.num_channels as usize * config.num_frames];
    let mut max_execution_time = Duration::ZERO;
    let deadline = period.saturating_sub(safety_margin);

    while !stop.load(Ordering::Relaxed) {
        let process_start = Instant::now();
        if let Err(err) = processor.process_interleaved(&mut buffer).await {
            return SessionReport {
                session_id,
                max_execution_time,
                error: Some(format!("process error: {}", err)),
            };
        }

        let execution_time = process_start.elapsed();
        max_execution_time = max_execution_time.max(execution_time);

        if execution_time > deadline {
            return SessionReport {
                session_id,
                max_execution_time,
                error: Some(format!("late by {:?}", execution_time - deadline)),
            };
        }

        let sleep_for = (process_start + period).saturating_duration_since(Instant::now());
        if !sleep_for.is_zero() {
            time::sleep(sleep_for).await;
        }
    }

    SessionReport {
        session_id,
        max_execution_time,
        error: None,
    }
}

fn number_suffix(n: usize) -> &'static str {
    match n % 10 {
        1 if n % 100 != 11 => "st",
        2 if n % 100 != 12 => "nd",
        3 if n % 100 != 13 => "rd",
        _ => "th",
    }
}
