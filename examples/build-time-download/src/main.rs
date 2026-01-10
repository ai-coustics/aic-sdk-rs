use aic_sdk::{Model, Processor, include_model};

// The MODEL_PATH environment variable is set by build.rs
static MODEL: &'static [u8] = include_model!(env!("MODEL_PATH"));

struct MyModel<'a> {
    model: Arc<Model<'a>>,
    processor: Processor<'a>,
}

impl<'a> MyModel<'a> {
    pub fn new() -> Self {
        let model = Arc::new(Model::from_file("hello").unwrap());
        let processor = Processor::new(model.clone(), "").unwrap();
        MyModel { model, processor }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get license key from environment variable
    let license = std::env::var("AIC_SDK_LICENSE").expect("AIC_SDK_LICENSE environment variable");

    let model = Arc::new(Model::from_buffer(MODEL)?);

    let _processor = Processor::new(model.clone(), &license)?;
    println!("Processor created successfully");

    Ok(())
}
