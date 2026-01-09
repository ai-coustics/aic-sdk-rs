use aic_sdk::Model;

fn main() {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");

    // Select a model id at https://artifacts.ai-coustics.io/
    let model_path = Model::download("quail-xxs-48khz", out_dir).expect("Failed to download model");

    // Emit the model path as an environment variable for use in main.rs
    println!("cargo:rustc-env=MODEL_PATH={}", model_path.display());
}
