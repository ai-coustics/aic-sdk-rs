#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use aic_sdk_sys::*;

#[derive(Debug)]
pub enum AicError {
    Failed,
    FailedNullPointer,
    Unknown,
}

impl Into<AicError> for u32 {
    fn into(self) -> AicError {
        match self {
            aic_AIC_FAIL => AicError::Failed,
            aic_AIC_FAIL_NULL_POINTER => AicError::FailedNullPointer,
            _ => AicError::Unknown,
        }
    }
}

fn handle_error(error_code: u32) -> Result<(), AicError> {
    match error_code {
        aic_AIC_PASS => Ok(()),
        _ => Err(error_code.into()),
    }
}

pub enum AicModelType {
    /// Our standard real-time speech enhancement model.
    ModelL,
    /// Super lightweight model for the price of more artefacts.
    ModelS,
}

/// The AicModel machine learning model for speech enhancement.
pub struct AicModel {
    client: *mut aic_AicModel,
}

impl AicModel {
    pub fn new(model_type: AicModelType, license_key: &str) -> Result<Self, AicError> {
        let client: *mut aic_AicModel;
        unsafe {
            handle_error(aic_aic_register_license_key(
                license_key.as_ptr(),
                license_key.len(),
            ))?;

            match model_type {
                AicModelType::ModelL => client = aic_aic_new_model_l(),
                AicModelType::ModelS => client = aic_aic_new_model_s(),
            }
        }

        Ok(Self { client })
    }

    /// Initializes the audio processing settings.
    /// This function has to be called once before the processing can begin.
    pub fn initialize(
        &mut self,
        num_channels: usize,
        sample_rate: usize,
        num_frames: usize,
    ) -> Result<(), AicError> {
        unsafe {
            handle_error(aic_aic_init(
                self.client,
                num_channels,
                sample_rate,
                num_frames,
            ))
        }
    }

    /// Processes a buffer of interleaved audio data.
    pub fn process_interleaved(
        &mut self,
        buffer: &mut [f32],
        num_channels: usize,
        num_frames: usize,
    ) -> Result<(), AicError> {
        unsafe {
            handle_error(aic_aic_process_interleaved(
                self.client,
                buffer.as_mut_ptr(),
                num_channels,
                num_frames,
            ))
        }
    }

    /// Processes a buffer of audio data where every channel is in a seperate buffer.
    /// Currently only up to 16 channels.
    pub fn process_deinterleaved<T>(&mut self, buffers: &mut [T]) -> Result<(), AicError>
    where
        T: AsMut<[f32]>,
    {
        let num_channels = buffers.len();
        if num_channels > 16 {
            return Err(AicError::Failed);
        }
        let num_frames = buffers.first_mut().ok_or(AicError::Failed)?.as_mut().len();

        // Create a fixed-size array on the stack with 16 channels
        let mut ptrs = [std::ptr::null_mut(); 16];
        for (i, buffer) in buffers.iter_mut().enumerate() {
            ptrs[i] = buffer.as_mut().as_mut_ptr();
        }

        unsafe {
            handle_error(aic_aic_process_deinterleaved(
                self.client,
                ptrs.as_mut_ptr(),
                num_channels,
                num_frames,
            ))
        }
    }

    /// Resets all states of the model.
    pub fn reset(&mut self) -> Result<(), AicError> {
        unsafe { handle_error(aic_aic_reset(self.client)) }
    }

    /// Sets the enhancement strength.
    /// Value can be between 0.0 and 1.0, where 0.0 is no enhancement and 1.0 is full enhancement.
    pub fn set_enhancement_strength(&mut self, enhancement_strength: f32) -> Result<(), AicError> {
        handle_error(unsafe { aic_aic_set_enhancement_strength(self.client, enhancement_strength) })
    }

    /// Gets the current enhancement strength.
    pub fn enhancement_strength(&mut self) -> Result<f32, AicError> {
        let mut enhancement_strength = 0.0;
        handle_error(unsafe {
            aic_aic_get_enhancement_strength(self.client, &mut enhancement_strength)
        })?;
        Ok(enhancement_strength)
    }

    /// Sets the gain of the enhanced voice.
    /// Value can be any linear gain value, where 1.0 does not change the gain,
    /// a larger value increases the level and a smaller value decreases the level.
    pub fn set_voice_gain(&mut self, voice_gain: f32) -> Result<(), AicError> {
        handle_error(unsafe { aic_aic_set_voice_gain(self.client, voice_gain) })
    }

    /// Gets the current gain of the enhanced voice.
    pub fn voice_gain(&mut self) -> Result<f32, AicError> {
        let mut voice_gain = 0.0;
        handle_error(unsafe { aic_aic_get_voice_gain(self.client, &mut voice_gain) })?;
        Ok(voice_gain)
    }

    /// Gets the optimal number of frames of the model.
    /// This is the native number of frames of the model, that causes
    /// the lowest latency.
    pub fn optimal_num_frames(&mut self) -> Result<usize, AicError> {
        let mut num_frames = 0;
        handle_error(unsafe { aic_aic_get_optimal_num_frames(self.client, &mut num_frames) })?;
        Ok(num_frames)
    }

    /// Gets the optimal sample rarte of the model.
    /// This is the native sample rate of the model, that causes
    /// the lowest latency.
    pub fn optimal_sample_rate(&mut self) -> Result<usize, AicError> {
        let mut sample_rate = 0;
        handle_error(unsafe { aic_aic_get_optimal_sample_rate(self.client, &mut sample_rate) })?;
        Ok(sample_rate)
    }

    /// Gets the currently applied latency/delay to the audio stream.
    pub fn latency(&mut self) -> Result<usize, AicError> {
        let mut latency = 0;
        handle_error(unsafe { aic_aic_get_latency(self.client, &mut latency) })?;
        Ok(latency)
    }
}

impl Drop for AicModel {
    fn drop(&mut self) {
        unsafe {
            aic_aic_free(self.client);
        }
    }
}

#[test]
fn test_aic_sdk() {
    let licence_key = std::env::var("AIC_SDK_LICENSE").unwrap();
    let mut aic = AicModel::new(AicModelType::ModelL, &licence_key).unwrap();

    aic.initialize(1, 48000, 512).unwrap();

    let mut buffer = vec![1.0; 512];
    aic.process_interleaved(&mut buffer, 1, 512).unwrap();
    aic.process_interleaved(&mut buffer, 1, 512).unwrap();
    assert_ne!(buffer, [1.0; 512]);
    assert_ne!(buffer, [0.0; 512]);

    let mut buffers = vec![vec![1.0; 512]];
    aic.process_deinterleaved(&mut buffers).unwrap();
    aic.process_deinterleaved(&mut buffers).unwrap();
    assert_ne!(buffer, [1.0; 512]);
    assert_ne!(buffer, [0.0; 512]);
}
