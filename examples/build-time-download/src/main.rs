use aic_sdk::{Model, Processor, include_model};

// The MODEL_PATH environment variable is set by build.rs
static MODEL: &'static [u8] = include_model!(env!("MODEL_PATH"));

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get license key from environment variable
    let license = std::env::var("AIC_SDK_LICENSE").map_err(|_| {
        eprintln!("Error: AIC_SDK_LICENSE environment variable not set");
        eprintln!("Please set it with: export AIC_SDK_LICENSE=your_license_key");
        std::io::Error::new(std::io::ErrorKind::NotFound, "AIC_SDK_LICENSE not set")
    })?;

    let model = Model::from_buffer(MODEL)?;

    let _processor = Processor::new(&model, &license)?;
    println!("Processor created successfully");

    Ok(())
}
