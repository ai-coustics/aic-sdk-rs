### New Features

#### SDK-internal error reporting

The SDK reports its own backend failures to ai-coustics error tracking. Covered are failed session
activations, failed usage reports, and bearer token refreshes rejected by
`Processor::update_bearer_token`, `Vad::update_bearer_token` and `Analyzer::update_bearer_token`.

A report contains the error class and message, the SDK version and wrapper, the model ID, the
operating system, the CPU architecture, and the account the license was issued to. It contains no
audio, no license key and no bearer token.

Disable reporting with `DO_NOT_TRACK=1`. The variable is read once per process. Licenses with an
offline entitlement and `wasm32` builds never report.
