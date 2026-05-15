import type { BinaryStreamOf } from './app-render';
import type { Readable } from 'node:stream';
/**
 * Render Flight stream.
 * This is only used for renderToHTML, the Flight response does not need additional wrappers.
 */
export declare function getFlightStream<T>(flightStream: Readable | BinaryStreamOf<T>, debugStream: Readable | ReadableStream<Uint8Array> | undefined, debugEndTime: number | undefined, nonce: string | undefined): Promise<T>;
/**
 * Creates a ReadableStream provides inline script tag chunks for writing hydration
 * data to the client outside the React render itself.
 *
 * @param flightStream The RSC render stream
 * @param nonce optionally a nonce used during this particular render
 * @param formState optionally the formState used with this particular render
 * @returns a ReadableStream without the complete property. This signifies a lazy ReadableStream
 */
export declare function createInlinedDataReadableStream(flightStream: ReadableStream<Uint8Array>, nonce: string | undefined, formState: unknown | null): ReadableStream<Uint8Array>;
