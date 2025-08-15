/**
 * This file contains the definitions and declarations for the ai-coustics
 * speech enhancement SDK, including initialization, processing, and configuration
 * functions. The ai-coustics SDK provides advanced machine learning models for
 * speech enhancement, that can be used in audio streaming contexts.
 *
 * Copyright (C) ai-coustics GmbH - All Rights Reserved
 *
 * Unauthorized copying, distribution, or modification of this file,
 * via any medium, is strictly prohibited.
 *
 * For inquiries, please contact: systems@ai-coustics.com
 */


#ifndef AIC_H
#define AIC_H

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

typedef enum AicErrorCode {
  /**
   * Operation completed successfully
   */
  AIC_ERROR_CODE_SUCCESS = 0,
  /**
   * Required pointer argument was NULL
   */
  AIC_ERROR_CODE_NULL_POINTER = 1,
  /**
   * License key format is invalid or corrupted
   */
  AIC_ERROR_CODE_LICENSE_INVALID = 2,
  /**
   * License key has expired
   */
  AIC_ERROR_CODE_LICENSE_EXPIRED = 3,
  /**
   * Audio configuration is not supported by the model
   */
  AIC_ERROR_CODE_UNSUPPORTED_AUDIO_CONFIG = 4,
  /**
   * Process was called with a different audio buffer configuration than initialized
   */
  AIC_ERROR_CODE_AUDIO_CONFIG_MISMATCH = 5,
  /**
   * Model must be initialized before this operation
   */
  AIC_ERROR_CODE_NOT_INITIALIZED = 6,
  /**
   * Parameter value is outside acceptable range
   */
  AIC_ERROR_CODE_PARAMETER_OUT_OF_RANGE = 7,
  /**
   * SDK activation failed
   */
  AIC_ERROR_CODE_SDK_ACTIVATION_ERROR = 8,
} AicErrorCode;

/**
 * Available model types for audio enhancement.
 */
typedef enum AicModelType {
  /**
   * **Specifications:**
   * - Native sample rate: 48 kHz
   * - Native num frames: 480
   * - Processing latency: 30ms
   */
  AIC_MODEL_TYPE_QUAIL_L48 = 0,
  /**
   * **Specifications:**
   * - Native sample rate: 16 kHz
   * - Native num frames: 160
   * - Processing latency: 30ms
   */
  AIC_MODEL_TYPE_QUAIL_L16 = 1,
  /**
   * **Specifications:**
   * - Native sample rate: 8 kHz
   * - Native num frames: 80
   * - Processing latency: 30ms
   */
  AIC_MODEL_TYPE_QUAIL_L8 = 2,
  /**
   * **Specifications:**
   * - Native sample rate: 48 kHz
   * - Native num frames: 480
   * - Processing latency: 30ms
   */
  AIC_MODEL_TYPE_QUAIL_S48 = 3,
  /**
   * **Specifications:**
   * - Native sample rate: 16 kHz
   * - Native num frames: 160
   * - Processing latency: 30ms
   */
  AIC_MODEL_TYPE_QUAIL_S16 = 4,
  /**
   * **Specifications:**
   * - Native sample rate: 8 kHz
   * - Native num frames: 80
   * - Processing latency: 30ms
   */
  AIC_MODEL_TYPE_QUAIL_S8 = 5,
  /**
   * **Specifications:**
   * - Native sample rate: 48 kHz
   * - Native num frames: 480
   * - Processing latency: 10ms
   */
  AIC_MODEL_TYPE_QUAIL_XS = 6,
  /**
   * **Specifications:**
   * - Native sample rate: 48 kHz
   * - Native num frames: 480
   * - Processing latency: 10ms
   */
  AIC_MODEL_TYPE_QUAIL_XXS = 7,
} AicModelType;

/**
 * Configurable parameters for audio enhancement
 */
typedef enum AicParameter {
  /**
   * Controls the intensity of speech enhancement processing.
   *
   * **Range:** 0.0 to 1.0
   * - **0.0:** Bypass mode - original signal passes through unchanged
   * - **1.0:** Full enhancement - maximum noise reduction but also more audible artifacts
   *
   * **Default:** 1.0
   */
  AIC_PARAMETER_ENHANCEMENT_LEVEL = 0,
  /**
   * Compensates for perceived volume reduction after noise removal.
   *
   * **Range:** 0.1 to 4.0 (linear amplitude multiplier)
   * - **0.1:** Significant volume reduction (-20 dB)
   * - **1.0:** No gain change (0 dB, default)
   * - **2.0:** Double amplitude (+6 dB)
   * - **4.0:** Maximum boost (+12 dB)
   *
   * **Formula:** Gain (dB) = 20 × log₁₀(value)
   * **Default:** 1.0
   */
  AIC_PARAMETER_VOICE_GAIN = 1,
  /**
   * Enables/disables a noise gate as a post-processing step,
   * before passing the audio buffer to the model.
   *
   * **Valid values:** 0.0 or 1.0
   * - **0.0:** Noise gate disabled
   * - **1.0:** Noise gate enabled
   *
   * **Default:** 0.0
   */
  AIC_PARAMETER_NOISE_GATE_ENABLE = 2,
} AicParameter;

typedef struct AicModel AicModel;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * Creates a new audio enhancement model instance.
 *
 * Multiple models can be created to process different audio streams simultaneously
 * or to switch between different enhancement algorithms during runtime.
 *
 * # Parameters
 * - `model`: Receives the handle to the newly created model. Must not be NULL.
 * - `model_type`: Selects the enhancement algorithm variant.
 * - `license_key`: NULL-terminated string containing your license key. Must not be NULL.
 *
 * # Returns
 * - `Success`: Model created successfully
 * - `NullPointer`: `model` or `license_key` is NULL
 * - `LicenseInvalid`: License key format is incorrect
 * - `LicenseExpired`: License key has expired
 */
enum AicErrorCode aic_model_create(struct AicModel **model,
                                   enum AicModelType model_type,
                                   const char *license_key);

/**
 * Releases all resources associated with a model instance.
 *
 * After calling this function, the model handle becomes invalid.
 * This function is safe to call with NULL.
 *
 * # Parameters
 * - `model`: Model instance to destroy. Can be NULL.
 */
void aic_model_destroy(struct AicModel *model);

/**
 * Configures the model for a specific audio format.
 *
 * This function must be called before processing any audio.
 * For the lowest delay use the sample rate and frame size returned by
 * `aic_get_optimal_sample_rate` and `aic_get_optimal_num_frames`.
 *
 * # Parameters
 * - `model`: Model instance to configure. Must not be NULL.
 * - `sample_rate`: Audio sample rate in Hz (8000 - 192000).
 * - `num_channels`: Number of audio channels (1 for mono, 2 for stereo, etc.).
 * - `num_frames`: Number of samples per channel in each process call.
 *
 * # Returns
 * - `Success`: Configuration accepted
 * - `NullPointer`: `model` is NULL
 * - `UnsupportedAudioConfig`: Configuration is not supported
 *
 * # Warning
 * Do not call from audio processing threads as this allocates memory.
 *
 * # Note
 * All channels are mixed to mono for processing. To process channels
 * independently, create separate model instances.
 */
enum AicErrorCode aic_model_initialize(struct AicModel *model,
                                       uint32_t sample_rate,
                                       uint16_t num_channels,
                                       size_t num_frames);

/**
 * Clears all internal state and buffers.
 *
 * Call this when the audio stream is interrupted or when seeking
 * to prevent artifacts from previous audio content.
 *
 * The model stays initialized to the configured settings.
 *
 * # Parameters
 * - `model`: Model instance to reset. Must not be NULL.
 *
 * # Returns
 * - `Success`: State cleared successfully
 * - `NullPointer`: `model` is NULL
 *
 * # Thread Safety
 * Real-time safe. Can be called from audio processing threads.
 */
enum AicErrorCode aic_model_reset(struct AicModel *model);

/**
 * Processes audio with separate buffers for each channel (planar layout).
 *
 * Enhances speech in the provided audio buffers in-place.
 *
 * The planar function allows a maximum of 16 channels.
 *
 * # Parameters
 * - `model`: Initialized model instance. Must not be NULL.
 * - `audio`: Array of channel buffer pointers. Must not be NULL.
 * - `num_channels`: Number of channels (must match initialization).
 * - `num_frames`: Number of samples per channel (must match initialization).
 *
 * # Returns
 * - `Success`: Audio processed successfully
 * - `NullPointer`: `model` or `audio` is NULL
 * - `NotInitialized`: Model has not been initialized
 * - `AudioConfigMismatch`: Channel or frame count mismatch
 */
enum AicErrorCode aic_model_process_planar(struct AicModel *model,
                                           float *const *audio,
                                           uint16_t num_channels,
                                           size_t num_frames);

/**
 * Processes audio with interleaved channel data.
 *
 * Enhances speech in the provided audio buffer in-place.
 *
 * # Parameters
 * - `model`: Initialized model instance. Must not be NULL.
 * - `audio`: Interleaved audio buffer. Must not be NULL and exactly of size `num_channels` * `num_frames`.
 * - `num_channels`: Number of channels (must match initialization).
 * - `num_frames`: Number of frames (must match initialization).
 *
 * # Returns
 * - `Success`: Audio processed successfully
 * - `NullPointer`: `model` or `audio` is NULL
 * - `NotInitialized`: Model has not been initialized
 * - `AudioConfigMismatch`: Channel or frame count mismatch
 */
enum AicErrorCode aic_model_process_interleaved(struct AicModel *model,
                                                float *audio,
                                                uint16_t num_channels,
                                                size_t num_frames);

/**
 * Modifies a model parameter.
 *
 * All parameters can be changed during audio processing.
 * This function can be called from any thread.
 *
 * # Parameters
 * - `model`: Model instance. Must not be NULL.
 * - `parameter`: Parameter to modify.
 * - `value`: New parameter value. See parameter documentation for ranges.
 *
 * # Returns
 * - `Success`: Parameter updated successfully
 * - `NullPointer`: `model` is NULL
 * - `ParameterOutOfRange`: Value outside valid range
 */
enum AicErrorCode aic_model_set_parameter(struct AicModel *model,
                                          enum AicParameter parameter,
                                          float value);

/**
 * Retrieves the current value of a parameter.
 *
 * This function can be called from any thread.
 *
 * # Parameters
 * - `model`: Model instance. Must not be NULL.
 * - `parameter`: Parameter to query.
 * - `value`: Receives the current parameter value. Must not be NULL.
 *
 * # Returns
 * - `Success`: Parameter retrieved successfully
 * - `NullPointer`: `model` or `value` is NULL
 */
enum AicErrorCode aic_model_get_parameter(const struct AicModel *model,
                                          enum AicParameter parameter,
                                          float *value);

/**
 * Returns the total output delay in samples for the current audio configuration.
 *
 * This function provides the complete end-to-end latency introduced by the model,
 * which includes both algorithmic processing delay and any buffering overhead.
 * Use this value to synchronize enhanced audio with other streams or to implement
 * delay compensation in your application.
 *
 * **Delay behavior:**
 * - **Before initialization:** Returns the base processing delay using the model's
 *   optimal frame size at its native sample rate
 * - **After initialization:** Returns the actual delay for your specific configuration,
 *   including any additional buffering introduced by non-optimal frame sizes
 *
 * **Important:** The delay value is always expressed in samples at the sample rate
 * you configured during `aic_model_initialize`. To convert to time units:
 * `delay_ms = (delay_samples * 1000) / sample_rate`
 *
 * **Note:** Using frame sizes different from the optimal value returned by
 * `aic_get_optimal_num_frames` will increase the delay beyond the model's base latency.
 *
 * # Parameters
 * - `model`: Initialized model instance. Must not be NULL.
 * - `delay`: Receives the delay in samples. Must not be NULL.
 *
 * # Returns
 * - `Success`: Latency retrieved successfully
 * - `NullPointer`: `model` or `latency` is NULL
 */
enum AicErrorCode aic_get_output_delay(const struct AicModel *model, size_t *delay);

/**
 * Retrieves the native sample rate of the selected model.
 *
 * Each model is optimized for a specific sample rate, which determines the frequency
 * range of the enhanced audio output. While you can process audio at any sample rate,
 * understanding the model's native rate helps predict the enhancement quality.
 *
 * **How sample rate affects enhancement:**
 * - Models trained at lower sample rates (e.g., 8 kHz) can only enhance frequencies
 *   up to their Nyquist limit (4 kHz for 8 kHz models)
 * - When processing higher sample rate input (e.g., 48 kHz) with a lower-rate model,
 *   only the lower frequency components will be enhanced
 *
 * **Enhancement blending:**
 * When enhancement strength is set below 1.0, the enhanced signal is blended with
 * the original, maintaining the full frequency spectrum of your input while adding
 * the model's noise reduction capabilities to the lower frequencies.
 *
 * **Sample rate and optimal frames relationship:**
 * When using different sample rates than the model's native rate, the optimal number
 * of frames (returned by `aic_get_optimal_num_frames`) will change. The model's output
 * delay remains constant regardless of sample rate as long as you use the optimal frame
 * count for that rate.
 *
 * **Recommendation:**
 * For maximum enhancement quality across the full frequency spectrum, match your
 * input sample rate to the model's native rate when possible.
 *
 * # Parameters
 * - `model`: Model instance. Must not be NULL.
 * - `sample_rate`: Receives the optimal sample rate in Hz. Must not be NULL.
 *
 * # Returns
 * - `Success`: Sample rate retrieved successfully
 * - `NullPointer`: `model` or `sample_rate` is NULL
 */
enum AicErrorCode aic_get_optimal_sample_rate(const struct AicModel *model, uint32_t *sample_rate);

/**
 * Retrieves the native number of frames for the selected model and sample rate.
 *
 * Using the optimal number of frames minimizes latency by avoiding internal buffering.
 * **When you use a different frame count than the optimal value, the model will
 * introduce additional buffering latency on top of its base processing delay.**
 *
 * The optimal frame count adjusts dynamically based on the sample rate used during
 * initialization. Each time you call `aic_model_initialize` with a different sample rate,
 * the optimal number of frames will update accordingly. Before initialization is called,
 * this function returns the optimal frame count for the model's native sample rate.
 *
 * Each model operates on a fixed time window duration, so the required number of frames
 * varies with sample rate. For example, a model designed for 10 ms processing windows
 * requires 480 frames at 48 kHz, but only 160 frames at 16 kHz to capture the same
 * duration of audio.
 *
 * # Parameters
 * - `model`: Model instance. Must not be NULL.
 * - `num_frames`: Receives the optimal frame count. Must not be NULL.
 *
 * # Returns
 * - `Success`: Frame count retrieved successfully
 * - `NullPointer`: `model` or `num_frames` is NULL
 */
enum AicErrorCode aic_get_optimal_num_frames(const struct AicModel *model, size_t *num_frames);

/**
 * Returns the version of the SDK.
 *
 * # Safety
 * The returned pointer points to a static string and remains valid
 * for the lifetime of the program. The caller should NOT free this pointer.
 *
 * # Returns
 * A null-terminated C string containing the version (e.g., "1.2.3")
 */
const char *aic_get_sdk_version(void);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* AIC_H */
