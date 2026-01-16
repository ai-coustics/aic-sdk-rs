use aic_sdk::{Model, Processor, ProcessorConfig};
use std::{
    env,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, watch};

const MODEL: &str = "quail-vf-l-16khz";

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

    let period = config.num_frames as f64 / config.sample_rate as f64;
    let period = Duration::from_secs_f64(period);

    println!("Model: {}", model.id());
    println!("Sample rate: {} Hz", config.sample_rate);
    println!("Frames per buffer: {}", config.num_frames);
    println!("Period: {} ms\n", period.as_millis());

    println!(
        "Starting benchmark: spawning a session every 5 seconds until a deadline is missed...\n"
    );

    let (stop_tx, stop_rx) = watch::channel(false);
    let (report_tx, mut report_rx) = mpsc::unbounded_channel::<SessionReport>();
    let mut active_sessions = 0usize;

    let mut handles = Vec::new();
    let mut session_id = 1usize;

    handles.push(spawn_session(
        session_id,
        Arc::clone(&model),
        license.clone(),
        config.clone(),
        period,
        stop_rx.clone(),
        report_tx.clone(),
    ));

    println!("Started session {session_id}");
    active_sessions += 1;

    let spawn_interval = Duration::from_secs(5);
    let mut next_spawn = tokio::time::Instant::now() + spawn_interval;

    let mut reports = Vec::new();
    let first_session_report = loop {
        tokio::select! {
            // Spawn a new session at regular intervals
            _ = tokio::time::sleep_until(next_spawn) => {
                session_id += 1;
                handles.push(spawn_session(
                    session_id,
                    Arc::clone(&model),
                    license.clone(),
                    config.clone(),
                    period,
                    stop_rx.clone(),
                    report_tx.clone(),
                ));
                active_sessions += 1;
                println!("Started session {session_id}");
                next_spawn += spawn_interval;
            }
            // Check for deadline misses and break the loop if one occurs
            Some(report) = report_rx.recv() => {
                let is_miss = report.error.is_some();
                reports.push(report);
                if is_miss {
                    break reports.last().cloned();
                }
            }
        }
    };

    println!("Benchmark complete\n");

    let _ = stop_tx.send(true);
    drop(report_tx);
    for handle in handles {
        let _ = handle.await;
    }

    while let Some(report) = report_rx.recv().await {
        reports.push(report);
    }
    reports.sort_by_key(|report| report.session_id);

    let mut number_of_missed_deadlines = 0;

    println!("\nSession report (max processing time per buffer):");
    for report in &reports {
        let max_ms = report.max_execution_time.as_secs_f64() * 1000.0;
        let period_ms = period.as_secs_f64() * 1000.0;

        let rtf = if period_ms > 0.0 {
            max_ms / period_ms
        } else {
            0.0
        };

        let miss_note = match report.error.as_deref() {
            Some(reason) => {
                number_of_missed_deadlines += 1;
                format!(" (deadline missed: {})", reason)
            }
            None => String::new(),
        };

        println!(
            "Session {:>3}: max {:>7.3} ms (RTF: {:>6.3}){}",
            report.session_id, max_ms, rtf, miss_note
        );
    }

    println!();

    let max_ok = active_sessions.saturating_sub(1);
    if let Some(first_session_report) = &first_session_report {
        println!(
            "After spawning {} concurrent sessions, session {} missed its deadline ({})",
            active_sessions,
            first_session_report.session_id,
            first_session_report.error.as_deref().unwrap_or("unknown")
        );

        if number_of_missed_deadlines > 1 {
            println!(
                "Other sessions also missed deadlines after session {}",
                first_session_report.session_id
            );
        }
    } else {
        println!("Missed deadline in session unknown (no report)");
    }

    println!(
        "Max concurrent sessions without missing deadlines: {}",
        max_ok
    );

    Ok(())
}

fn spawn_session(
    session_id: usize,
    model: Arc<Model<'static>>,
    license: String,
    config: ProcessorConfig,
    period: Duration,
    stop_rx: watch::Receiver<bool>,
    report_tx: mpsc::UnboundedSender<SessionReport>,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        let mut processor =
            match Processor::new(&model, &license).and_then(|p| p.with_config(&config)) {
                Ok(processor) => processor,
                Err(err) => {
                    let reason = format!("processor init failed: {}", err);
                    let _ = report_tx.send(SessionReport {
                        session_id,
                        max_execution_time: Duration::from_secs(0),
                        error: Some(reason),
                    });
                    return;
                }
            };

        let mut buffer = vec![0.0f32; config.num_channels as usize * config.num_frames];

        let mut max_execution_time = Duration::from_secs(0);
        let mut error = None;

        loop {
            // Check if we should stop (another session missed a deadline)
            if *stop_rx.borrow() {
                break;
            }

            // Process the audio buffer
            let process_start = Instant::now();
            if let Err(err) = processor.process_interleaved(&mut buffer) {
                error = Some(format!("process error: {}", err));
                break;
            }
            let execution_time = process_start.elapsed();

            // Keep track of the maximum execution time
            if execution_time > max_execution_time {
                max_execution_time = execution_time;
            }

            // Check if we missed the deadline
            if execution_time > period {
                let late_by = execution_time - period;
                let reason = format!("late by {:?}", late_by);
                error = Some(reason);
                break;
            }

            // Sleep until the next deadline
            let next_deadline = process_start + period;
            let sleep_for = next_deadline.saturating_duration_since(Instant::now());
            if sleep_for > Duration::from_secs(0) {
                std::thread::sleep(sleep_for);
            }
        }

        // Send the session report
        let _ = report_tx.send(SessionReport {
            session_id,
            max_execution_time,
            error,
        });
    })
}
