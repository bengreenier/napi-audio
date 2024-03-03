# @napi-audio/decoder-stream

> For information about supported codecs, containers, and runtime platforms see [@napi-audio/decoder](../decoder).

A streaming interface for [@napi-audio/decoder](../decoder).

### Getting Started

```
# With npm
npm install @napi-audio/decoder-stream

# With pnpm
pnpm install @napi-audio/decoder-stream

# With yarn
yarn add @napi-audio/decoder-stream
```

#### Usage

```js
import { createReadStream, createWriteStream } from "node:fs";
import { DecoderStream } from "@napi-audio/decoder-stream";

// create a read stream from an audio file
createReadStream("./audio-file.mp3")
  // pipe the stream into an instance of DecoderStream
  .pipe(
    new DecoderStream({
      // don't enable gapless decoding (optional, disabled by default)
      enableGapless: false,
      // a hint about the audio format (optional, recommended)
      mimeType: "audio/mpeg3",
      // a hint about the audio container (optional, recommended)
      fileExtension: "mp3",
      // Callback to be invoked when the decoder detects the metadata of the underlying audio.
      // this is optional, but highly recommended if you're doing additional audio processing.
      onMetadataDetected(channelCount, sampleRate) {
        console.log(
          `Audio stream contains ${channelCount} channels @ ${sampleRate}hz`
        );
      },
    })
  )
  // pipe the decoded audio to an output file stream
  // note that the format is always interleaved PCM audio (Signed 16-bit PCM, Little-Endian)
  .pipe(createWriteStream("./audio-file.pcm"));

// The generated file can be inspected with Audacity, using File => Import => Raw Data
```

#### Debugging

The native library is written in Rust, and can output tracing information if configured to do so.
The `setNativeTracing` export can be used for this - pass `true` to enable logging, or `false` to disable.
You'll also need to ensure the `RUST_LOG` environment variable is set according to the [EnvFilter docs](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html); for example `RUST_LOG=trace`. Note that tracing disabled by default.

### Licensing

#### Source Code

This source is licensed under [MPL-2.0](https://www.mozilla.org/en-US/MPL/2.0/).

#### Codecs

> I am a software developer, not a lawyer. If you're using this in production you should consult a lawyer.

If you're using this library to parse certain media formats and codecs, **you may need to pay royalties**. For more information, see [Symphonia Docs](https://docs.rs/symphonia/latest/symphonia/) and/or [this Google search](https://www.google.com/search?q=audio+codec+royalties).
