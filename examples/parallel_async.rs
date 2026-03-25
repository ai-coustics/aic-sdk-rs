// Demonstrates that multiple `ProcessorAsync` instances genuinely run in
// parallel when awaited concurrently.
//
// Each processor records its own wall-clock processing time.  If they ran
// sequentially, the total elapsed time would be roughly `N × per-processor time`.
// When running in parallel the total time is close to the slowest
// single processor, which is what we verify and print.

use aic_sdk::{Model, ProcessorAsync, ProcessorConfig};
use std::time::Instant;

const MODEL: &str = "quail-vf-2.0-l-16khz";
const NUM_PROCESSORS: usize = 4;
// Number of process calls per processor – enough to make timing visible.
const ITERATIONS: usize = 50;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("ai-coustics SDK version: {}", aic_sdk::get_sdk_version());

    let license = std::env::var("AIC_SDK_LICENSE").expect("AIC_SDK_LICENSE not set");

    let model_path = Model::download(MODEL, "target")?;
    let model = Model::from_file(&model_path)?;
    println!("Model loaded from {}", model_path.display());

    let config = ProcessorConfig::optimal(&model);
    println!(
        "Config: {} Hz, {} frames/buffer, {} channel(s)\n",
        config.sample_rate, config.num_frames, config.num_channels
    );

    // -------------------------------------------------------------------------
    // Build all processors upfront so initialization is not part of the timed
    // section.
    // -------------------------------------------------------------------------
    let mut processors = futures::future::try_join_all(
        (0..NUM_PROCESSORS).map(|_| ProcessorAsync::with_config(&model, &license, &config)),
    )
    .await?;

    println!(
        "Running {} processors × {} iterations each",
        NUM_PROCESSORS, ITERATIONS
    );

    // -------------------------------------------------------------------------
    // Sequential baseline – process each processor one after the other.
    // -------------------------------------------------------------------------
    let buf_len = config.num_channels as usize * config.num_frames;

    let sequential_start = Instant::now();
    for p in &mut processors {
        let mut audio = vec![0.0f32; buf_len];
        for _ in 0..ITERATIONS {
            p.process_interleaved(&mut audio).await?;
        }
    }
    let sequential_elapsed = sequential_start.elapsed();

    println!(
        "Sequential total:  {:>8.1} ms",
        sequential_elapsed.as_secs_f64() * 1000.0
    );

    // -------------------------------------------------------------------------
    // Parallel run – drive all processors concurrently with futures::future::try_join_all.
    // Each task times itself and returns its own elapsed duration.
    // -------------------------------------------------------------------------
    let parallel_start = Instant::now();

    let tasks: Vec<_> = processors
        .iter_mut()
        .map(|p| {
            let config = config.clone();
            async move {
                let mut audio = vec![0.0f32; config.num_channels as usize * config.num_frames];
                let t0 = Instant::now();
                for _ in 0..ITERATIONS {
                    p.process_interleaved(&mut audio).await?;
                }
                let elapsed = t0.elapsed();
                Ok::<_, aic_sdk::AicError>(elapsed)
            }
        })
        .collect();

    let results = futures::future::try_join_all(tasks).await?;
    let parallel_elapsed = parallel_start.elapsed();

    for (id, elapsed) in results.iter().enumerate() {
        println!(
            "  Processor {:>2} finished in {:>8.1} ms",
            id + 1,
            elapsed.as_secs_f64() * 1000.0,
        );
    }

    let max_individual = results.iter().max().copied().unwrap_or_default();

    println!(
        "\nParallel total:      {:>8.1} ms",
        parallel_elapsed.as_secs_f64() * 1000.0
    );
    println!(
        "Slowest processor:   {:>8.1} ms",
        max_individual.as_secs_f64() * 1000.0,
    );

    let speedup = sequential_elapsed.as_secs_f64() / parallel_elapsed.as_secs_f64();
    println!(
        "\nSpeedup vs sequential: {:.2}x  (ideal ≈ {}x)",
        speedup, NUM_PROCESSORS
    );
    println!(
        "{}",
        if speedup > 1.5 {
            "Parallel execution confirmed."
        } else {
            "Warning: low speedup – your system may not have enough CPU cores for parallel blocking tasks."
        }
    );

    Ok(())
}
