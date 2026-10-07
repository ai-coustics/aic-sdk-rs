### Improvements

- Improved the robustness of the `interfering_speech` dimension in Tyto 1.1.

### Bug Fixes

- Fixed a click and brief drop in audio volume with Quail Voice Focus models when speech is first detected after processing starts or the processor is reset.
- Quail Voice Focus models now apply the requested enhancement level (`ProcessorParameter::EnhancementLevel`) even before speech is first detected. Previously, all nonzero enhancement levels produced the same audio during this initial period.
- Fixed `terminate_session` blocking until sessions created after the call ended. Terminating the last session now waits only for the final usage reports of sessions that were already terminated. With `ProcessorAsync` and `VadAsync`, these blocked calls could occupy every thread of the shared processing pool and stall all processing indefinitely.
