# timed-audio

This is a simple crate that allows to play audio at a specific time in the future, accounting for the latency of the audio device. Most changes were made upstream, so this crate is mostly a wrapper around the `cpal` crate.

Limitations:
 - Only one audio object can be played at a time. If you start a new audio object (either immediately or at a predefined time), the previous one will be stopped.
- Windows latency information might be unreliable when used in WASAPI shared mode.
