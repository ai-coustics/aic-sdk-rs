use aic_sdk::{Model, Processor, include_model};

// The MODEL_PATH environment variable is set by build.rs
static MODEL: &'static [u8] = include_model!(env!("MODEL_PATH"));

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get license key from environment variable
    let license = env::var("AIC_SDK_LICENSE").map_err(|_| "AIC_SDK_LICENSE not set")?;

    let model = Model::from_buffer(MODEL)?;

    let _processor = Processor::new(&model, &license)?;
    println!("Processor created successfully");

    Ok(())
}
