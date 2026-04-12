# marginalia-playback-host

Host-side playback engine using rodio for in-process audio on
macOS, Linux, and Windows.

Implements the `PlaybackEngine` trait with pause, resume, stop,
and playback-finished detection (`sink.empty()`). Includes a
callback hook for feeding TTS audio samples to the AEC pipeline
as a render reference signal.
