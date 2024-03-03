# @napi-audio/decoder

A native audio decoder for NAPI-compatible runtimes.

For a streaming interface, see [@napi-audio/decoder-stream](../decoder-stream/).

The following containers are supported.

| Format   | Feature Flag | Gapless\* | Default |
| -------- | ------------ | --------- | ------- |
| AIFF     | `aiff`       | Yes       | No      |
| CAF      | `caf`        | No        | No      |
| ISO/MP4  | `isomp4`     | No        | No      |
| MKV/WebM | `mkv`        | No        | Yes     |
| OGG      | `ogg`        | Yes       | Yes     |
| Wave     | `wav`        | Yes       | Yes     |

The following codecs are supported.

| Codec  | Feature Flag | Gapless | Default |
| ------ | ------------ | ------- | ------- |
| AAC-LC | aac          | No      | No      |
| ADPCM  | adpcm        | Yes     | Yes     |
| ALAC   | alac         | Yes     | No      |
| FLAC   | flac         | Yes     | Yes     |
| MP1    | mp1, mpa     | No      | No      |
| MP2    | mp2, mpa     | No      | No      |
| MP3    | mp3, mpa     | Yes     | No      |
| PCM    | pcm          | Yes     | Yes     |
| Vorbis | vorbis       | Yes     | Yes     |

The following Node versions are supported.

| Node10 | Node12 | Node14 | Node16 | Node18 | Node20 |
| ------ | ------ | ------ | ------ | ------ | ------ |
| ✓      | ✓      | ✓      | ✓      | ✓      | ✓      |

On the following platforms.

|            | i686 | x64 | aarch64 | arm |
| ---------- | ---- | --- | ------- | --- |
| Windows    | ✅   | ✅  | ✅      | -   |
| macOS      | -    | ✅  | ✅      | ✅  |
| Linux      | -    | ✅  | ✅      | ✅  |
| Linux musl | -    | ✅  | ✅      | -   |
| FreeBSD    | -    | ✅  | -       | -   |
| Android    | -    | -   | ✅      | ✅  |

### Getting Started

```
# With npm
npm install @napi-audio/decoder

# With pnpm
pnpm install @napi-audio/decoder

# With yarn
yarn add @napi-audio/decoder
```

#### Usage

```js
import { readFileSync } from "fs";
import { Decoder } from "@napi-audio/decoder";

// read audio data from a file
const audioData = readFileSync("./audio-file.mp3");

// create an instance of Decoder
const decoder = new Decoder({
  // don't enable gapless decoding (optional, disabled by default)
  enableGapless: false,
  // a hint about the audio format (optional, recommended)
  mimeType: "audio/mpeg3",
  // a hint about the audio container (optional, recommended)
  fileExtension: "mp3",
});

// Append the audio data to the decoder, getting back a possible sample
// note that this can be called repeatedly with any "chunk" of data
const maybe_sample = decoder.append(audioData);

// if the sample is not null, we can process the contents
if (maybe_sample) {
  console.log(`Audio sample obtained with ${maybe_sample.data.length} bytes`);
  console.log(
    `Audio sample has ${maybe_sample.channelCount} channels @ ${maybe_sample.sampleRate}hz`
  );
}

// repeat the previous "append" step as many times as needed

// inform the decoder that we've sent all the data
decoder.finalize();

// flush the decoder, ensuring that all data has been decoded and processed
const maybe_final_sample = decoder.flush();

if (maybe_final_sample) {
  console.log(
    `Final sample obtained with ${maybe_final_sample.data.length} bytes`
  );
}

// then close the decoder, freeing native resources
decoder.close();
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
