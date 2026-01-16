#![cfg_attr(not(feature = "download-model"), allow(dead_code, unused_imports))]

#[cfg(feature = "download-model")]
use aic_sdk::{Model, Processor, ProcessorConfig};
#[cfg(feature = "download-model")]
use std::{
    env,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
#[cfg(feature = "download-model")]
use tokio::sync::{mpsc, watch};

#[cfg(not(feature = "download-model"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err("Enable the `download-model` feature to run this example.".into())
}

const MODEL: &str = "quail-vf-l-16khz";

struct DeadlineMiss {
    session_id: usize,
    message: String,
}

#[cfg(feature = "download-model")]
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

    println!("Starting benchmark: spawning a session every 5 seconds until a deadline is missed...\n");

    let (stop_tx, stop_rx) = watch::channel(false);
    let (miss_tx, mut miss_rx) = mpsc::unbounded_channel::<DeadlineMiss>();
    let active_sessions = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();
    let mut session_id = 1usize;
    
    handles.push(spawn_session(
        session_id,
        Arc::clone(&model),
        license.clone(),
        config.clone(),
        period,
        stop_rx.clone(),
        miss_tx.clone(),
    ));
    
    println!("Started session {} (active: 1)", session_id);
    active_sessions.fetch_add(1, Ordering::SeqCst);

    let spawn_interval = Duration::from_secs(5);
    let mut next_spawn = tokio::time::Instant::now() + spawn_interval;

    let miss = loop {
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
                    miss_tx.clone(),
                ));
                let active = active_sessions.fetch_add(1, Ordering::SeqCst) + 1;
                println!("Started session {} (active: {})", session_id, active);
                next_spawn += spawn_interval;
            }
            // Check for deadline misses and break the loop if one occurs
            Some(miss) = miss_rx.recv() => break miss,
        }
    };

    let active_at_miss = active_sessions.load(Ordering::SeqCst);
    let max_ok = active_at_miss.saturating_sub(1);
    println!(
        "Missed deadline in session {} ({}).",
        miss.session_id, miss.message
    );
    println!(
        "Max concurrent sessions without missed deadlines: {}",
        max_ok
    );

    let _ = stop_tx.send(true);
    for handle in handles {
        let _ = handle.await;
    }

    Ok(())
}

fn spawn_session(
    session_id: usize,
    model: Arc<Model<'static>>,
    license: String,
    config: ProcessorConfig,
    period: Duration,
    stop_rx: watch::Receiver<bool>,
    miss_tx: mpsc::UnboundedSender<DeadlineMiss>,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        let mut processor = match Processor::new(&model, &license).and_then(|p| p.with_config(&config)) {
            Ok(processor) => processor,
            Err(err) => {
                let _ = miss_tx.send(DeadlineMiss {
                    session_id,
                    message: format!("processor init failed: {}", err),
                });
                return;
            }
        };

        let mut buffer = vec![0.0f32; config.num_channels as usize * config.num_frames];
        let mut deadline = Instant::now() + period;

        loop {
            if *stop_rx.borrow() {
                break;
            }

            if let Err(err) = processor.process_interleaved(&mut buffer) {
                let _ = miss_tx.send(DeadlineMiss {
                    session_id,
                    message: format!("process error: {}", err),
                });
                break;
            }

            // Check if we missed the deadline
            let now = Instant::now();
            if now > deadline {
                let late_by = now.duration_since(deadline);
                let _ = miss_tx.send(DeadlineMiss {
                    session_id,
                    message: format!("late by {:?}", late_by),
                });
                break;
            }

            // Sleep until the next deadline
            let sleep_for = deadline.saturating_duration_since(Instant::now());
            if sleep_for > Duration::from_secs(0) {
                std::thread::sleep(sleep_for);
            }

            // Advance the deadline by one period
            deadline += period;
        }
    })
}
