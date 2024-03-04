import { test } from "uvu";
import * as assert from "uvu/assert";
import { createReadStream } from "node:fs";
import { performance } from "node:perf_hooks";
import { DecoderStream, setNativeTracing } from "./index.js";

const OGG_EXAMPLE_PATH = "./test-fixtures/file_example_OOG_2MG.ogg";

setNativeTracing(true);

test("DecoderStream should decode", async () => {
  let metadataDetectedAt: number;

  const audioStream = createReadStream(OGG_EXAMPLE_PATH)
    .pipe(
      new DecoderStream({
        fileExtension: "ogg",
        mimeType: "audio/ogg",
        enableGapless: false,
        onMetadataDetected(channelCount, sampleRate) {
          assert.equal(channelCount, 2);
          assert.equal(sampleRate, 44100);

          assert.equal(this.channelCount, channelCount);
          assert.equal(this.sampleRate, sampleRate);

          metadataDetectedAt = performance.now();
        },
      })
    )
    .on("data", (_chunk) => {
      // just read data to ensure the stream flows
    });

  const streamEndedAt = await new Promise<number>((resolve, reject) => {
    audioStream.on("error", reject);
    audioStream.on("finish", () => {
      resolve(performance.now());
    });
  });

  assert.ok(metadataDetectedAt!);
  assert.ok(streamEndedAt - metadataDetectedAt > 0);
});

test.run();
