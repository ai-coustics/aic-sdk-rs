/**
 * @file aic.h
 * @brief Header file for the ai-coustics speech enhancement SDK.
 *
 * This file contains the definitions and declarations for the ai-coustics
 * speech enhancement SDK, including initialization, processing, and
 * configuration functions. The ai-coustics SDK provides advanced machine
 * learning models for speech enhancement, that can be used in audio streaming
 * contexts.
 *
 * @copyright
 * Copyright (C) ai-coustics GmbH - All Rights Reserved
 *
 * Unauthorized copying, distribution, or modification of this file,
 * via any medium, is strictly prohibited.
 *
 * For inquiries, please contact: info@ai-coustics.com
 */

#ifndef AIC_SDK_H
#define AIC_SDK_H

#include <cstddef>
#include <cstdint>

namespace aic {

/**
 * Indicates that a function call was successful.
 */
constexpr static const uint32_t AIC_PASS = 0;

/**
 * Indicates that a function call failed for an unspecified reason.
 */
constexpr static const uint32_t AIC_FAIL = 1;

/**
 * Indicates that a function call failed because a null pointer was passed as an
 * argument.
 */
constexpr static const uint32_t AIC_FAIL_NULL_POINTER = 2;

/**
 * Levels for the log callback.
 */
enum class LogLevel : uint32_t {
  Error = 1,
  Warn,
  Debug,
  Info,
  Trace,
};

/**
 * The AicModel machine learning model for speech enhancement.
 */
struct AicModel;

/**
 * The SDK will call this function to allow the user to receive logs.
 * The first parameter is a pointer to the log message, and the second parameter
 * is the level of that log.
 */
using LogCallback = void (*)(const char *, LogLevel);

extern "C" {

/**
 * @brief Set the license key for the SDK. This function has to be called before
 * any other function in the SDK.
 *
 * @param license_key A pointer to the license key data.
 * @param license_key_length The length of the license key data in bytes.
 * @return AIC_PASS on success, or AIC_FAIL if the license key is invalid.
 * @note This function must only be called once.
 */
uint32_t aic_register_license_key(const uint8_t *license_key,
                                  size_t license_key_length);

/**
 * @brief Initializes the SDK logger.
 *
 * @param log_callback A callback function that the SDK will use to send log
 * messages
 * @return AIC_PASS on success, or AIC_FAIL if the SDK logging system was
 * already initialized.
 * @note This function must only be called once.
 */
uint32_t aic_log_init(LogCallback log_callback);

/**
 * @brief Creates a new `AicModel`, using the machine learning model `Model S`.
 *
 * @return A pointer to the newly created `AicModel`.
 */
AicModel *aic_new_model_s();

/**
 * @brief Creates a new `AicModel`, using the machine learning model `Model L`.
 *
 * @return A pointer to the newly created `AicModel`.
 */
AicModel *aic_new_model_l();

/**
 * @brief Initializes the audio processing settings for the AicModel.
 *
 * @param model A pointer to the AicModel.
 * @param num_channels The number of audio channels being processed.
 * @param sample_rate The sample rate of the audio being processed.
 * @param num_frames The number of audio frames processed per callback.
 * @return AIC_PASS on success, or an error code on failure.
 * @note This function has to be called before the process function.
 */
uint32_t aic_init(AicModel *model, size_t num_channels, size_t sample_rate,
                  size_t num_frames);

/**
 * @brief Processes a buffer of interleaved audio data using the AicModel.
 *
 * @param model A pointer to the AicModel.
 * @param buffer A pointer to the audio data.
 * @param num_channels The number of channels.
 * @param num_frames The number of audio frames in the buffer.
 * @return AIC_PASS on success, or an error code on failure.
 * @note The buffer has to be `num_channels` * `num_frames` long.
 */
uint32_t aic_process_interleaved(AicModel *model, float *buffer,
                                 size_t num_channels, size_t num_frames);

/**
 * @brief Processes a multi-channel buffer of audio data using the AicModel.
 *
 * @param model A pointer to the AicModel.
 * @param buffer A pointer to the audio data.
 * @param num_channels The number of channels.
 * @param num_frames The number of audio frames in the buffer.
 * @return AIC_PASS on success, or an error code on failure.
 */
uint32_t aic_process_deinterleaved(AicModel *model, float *const *buffer,
                                   size_t num_channels, size_t num_frames);

/**
 * @brief Resets all states of the model.
 *
 * @param model A pointer to the AicModel.
 * @return AIC_PASS on success, or an error code on failure.
 */
uint32_t aic_reset(AicModel *model);

/**
 * @brief Sets the enhancement strength for the AicModel.
 *
 * @param model A pointer to the AicModel.
 * @param enhancement_strength Value between 0.0 and 1.0,
 * where 0.0 is equal to a bypass and 1.0 is the maximum enhancement.
 * @return AIC_PASS on success, or an error code on failure.
 */
uint32_t aic_set_enhancement_strength(AicModel *model,
                                      float enhancement_strength);

/**
 * @brief Gets the current enhancement strength for the AicModel.
 *
 * @param model A pointer to the AicModel.
 * @param enhancement_strength A pointer to the variable that will store the
 * current strength.
 * @return AIC_PASS on success, or an error code on failure.
 */
uint32_t aic_get_enhancement_strength(AicModel *model,
                                      float *enhancement_strength);

/**
 * @brief Sets the voice gain parameter of the AicModel.
 * This is the gain that is added to the extracted voice before
 * the mixback to the original signal is happening.
 *
 * @param model A pointer to the AicModel.
 * @param voice_gain The voice gain parameter to set.
 * @return AIC_PASS on success, or an error code on failure.
 */
uint32_t aic_set_voice_gain(AicModel *model, float voice_gain);

/**
 * @brief Gets the current voice gain parameter of the AicModel.
 * This is the gain that is added to the extracted voice before the
 * mixback to the original signal is happening.
 *
 * @param model A pointer to the AicModel.
 * @param voice_gain A pointer to the variable that will store the voice gain
 * parameter.
 * @return AIC_PASS on success, or an error code on failure.
 */
uint32_t aic_get_voice_gain(AicModel *model, float *voice_gain);

/**
 * @brief Gets the optimal number of frames for the AicModel.
 * This is the native number of frames of the model, that causes
 * the lowest latency.
 *
 * @param model A pointer to the AicModel.
 * @param num_frames A pointer to the variable that will store the optimal
 * number of frames.
 * @return AIC_PASS on success, or an error code on failure.
 */
uint32_t aic_get_optimal_num_frames(AicModel *model, size_t *num_frames);

/**
 * @brief Gets the optimal sample rate for the AicModel.
 * This is the native sample rate of the model,
 * that causes the lowest latency.
 *
 * @param model A pointer to the AicModel.
 * @param sample_rate A pointer to the variable that will store the optimal
 * sample rate.
 * @return AIC_PASS on success, or an error code on failure.
 */
uint32_t aic_get_optimal_sample_rate(AicModel *model, size_t *sample_rate);

/**
 * @brief Gets the current latency of the full process in samples.
 *
 * @param model A pointer to the AicModel.
 * @param latency A pointer to the variable that will store the latency value.
 * @return AIC_PASS on success, or an error code on failure.
 */
uint32_t aic_get_latency(AicModel *model, size_t *latency);

/**
 * @brief Frees the memory used by the AicModel.
 *
 * @param model A pointer to the AicModel.
 */
void aic_free(AicModel *model);

} // extern "C"

} // namespace aic

#endif // AIC_SDK_H
