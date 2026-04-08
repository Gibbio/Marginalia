# marginalia-playback-host

Host-side playback engine for Marginalia Beta desktop runtimes.

The first implementation is command-based:

- resolves a local player command template
- launches playback as a child process
- tracks basic playback state for pause, resume, stop, and seek

Default command detection currently prefers:

- `afplay` on macOS
- `aplay` on Linux
- `ffplay` as a fallback when available
