# napi-audio

A native audio stack for NAPI-compatible JS runtimes. 🔉🚂

Features:

- Decode audio samples into PCM format, stored in an `Int16Array`.
- Support [Streams](https://nodejs.org/api/stream.html) of audio samples.

For supported codecs, containers, and runtime platforms, see the [@napi-audio/decoder README](./packages/decoder/).

See [GitHub Issues](https://github.com/bengreenier/napi-audio/issues) for feature requests. Contributions welcome!

## Packages

- [@napi-audio/decoder](./packages/decoder/) - A native audio decoder for NAPI-compatible runtimes.
- [@napi-audio/decoder-stream](./packages/decoder-stream/) - A streaming interface for [@napi-audio/decoder](./packages/decoder/).
