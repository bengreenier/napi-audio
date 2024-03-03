import { Transform } from "node:stream";
import { Decoder } from "@napi-audio/decoder";
import type { TransformOptions } from "node:stream";
import type { DecodedAudioSample, DecoderConfig } from "@napi-audio/decoder";
import type { TransformCallback } from "stream";

export { setNativeTracing } from "@napi-audio/decoder";

/**
 * Internal base type for {@link DecoderConfig}.
 *
 * This omits some keys that collide with {@link TransformOptions}
 */
type BaseOptions = Omit<
  DecoderConfig,
  keyof Pick<DecoderConfig, "highWaterMark">
>;

/**
 * Options for {@link DecoderStream}.
 */
export interface DecoderStreamOptions extends BaseOptions, TransformOptions {
  /**
   * An optional callback that will be invoked when the decoder has processed
   * enough data to determine the audio stream metadata.
   * @param channelCount - see {@link DecoderStream.channelCount}.
   * @param sampleRate - see {@link DecoderStream.sampleRate}.
   */
  onMetadataDetected?: (
    this: DecoderStream,
    channelCount: number,
    sampleRate: number
  ) => void;
}

/**
 * A decoder that processes audio samples from various supported containers
 * and codecs into interleaved PCM audio (Signed 16-bit PCM, Little-Endian).
 */
export class DecoderStream extends Transform {
  /**
   * The underlying decoder itself.
   */
  private readonly decoder: Decoder;

  /**
   * Storage for {@link channelCount}.
   */
  private _channelCount?: number;

  /**
   * Storage for {@link sampleRate}.
   */
  private _sampleRate?: number;

  /**
   * Storage for {@link DecoderStreamOptions.onMetadataDetected}.
   */
  private _onMetadataDetected?: (
    channelCount: number,
    sampleRate: number
  ) => void;

  /**
   * The detected channel count of the input audio stream.
   *
   * Note: Will be `undefined` until the decoder has processed enough data
   * to determine such a value.
   */
  public get channelCount() {
    return this._channelCount;
  }

  /**
   * The detected sample rate of the input audio stream.
   *
   * Note: Will be `undefined` until the decoder has processed enough data
   * to determine such a value.
   */
  public get sampleRate() {
    return this._sampleRate;
  }

  constructor({
    enableGapless,
    fileExtension,
    mimeType,
    onMetadataDetected,
    ...transformOptions
  }: DecoderStreamOptions) {
    super(transformOptions);

    this._onMetadataDetected = onMetadataDetected;
    this.decoder = new Decoder({
      highWaterMark:
        transformOptions.highWaterMark ||
        transformOptions.writableHighWaterMark,
      enableGapless,
      fileExtension,
      mimeType,
    });
  }

  _destroy(
    error: Error | null,
    callback: (error?: Error | null | undefined) => void
  ): void {
    try {
      this.decoder.close();
    } catch (err) {
      callback(error ?? (err instanceof Error ? err : new Error(`${err}`)));
    }

    callback(error);
  }

  _transform(
    chunk: any,
    encoding: BufferEncoding,
    callback: TransformCallback
  ): void {
    const input = Buffer.from(chunk, encoding);

    try {
      const sample = this.decoder.append(input);

      this.processSample(sample);
    } catch (err) {
      return callback(err instanceof Error ? err : new Error(`${err}`));
    }

    callback();
  }

  _flush(callback: TransformCallback): void {
    try {
      const sample = this.decoder.flush();

      this.processSample(sample);
    } catch (err) {
      return callback(err instanceof Error ? err : new Error(`${err}`));
    }

    callback();
  }

  _final(callback: (error?: Error | null | undefined) => void): void {
    try {
      this.decoder.finalize();
    } catch (err) {
      return callback(err instanceof Error ? err : new Error(`${err}`));
    }

    callback();
  }

  private processSample(sample: DecodedAudioSample | null) {
    if (sample) {
      const shouldInvokeMetadataCallback = this._sampleRate == undefined;

      this._channelCount = sample.channelCount;
      this._sampleRate = sample.sampleRate;

      if (shouldInvokeMetadataCallback && this._onMetadataDetected) {
        this._onMetadataDetected.call(
          this,
          this._channelCount,
          this._sampleRate
        );
      }

      this.push(Buffer.from(sample.data.buffer));
    }
  }
}
