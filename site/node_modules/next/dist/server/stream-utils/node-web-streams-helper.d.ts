import type { ReactDOMServerReadableStream } from 'react-dom/server';
export declare function chainStreams<T>(...streams: ReadableStream<T>[]): ReadableStream<T>;
export declare function streamFromString(str: string): ReadableStream<Uint8Array>;
export declare function streamFromBuffer(chunk: Buffer): ReadableStream<Uint8Array>;
export declare function streamToUint8Array(stream: ReadableStream<Uint8Array>): Promise<Uint8Array>;
export declare function streamToBuffer(stream: ReadableStream<Uint8Array>): Promise<Buffer>;
export declare function streamToString(stream: ReadableStream<Uint8Array>, signal?: AbortSignal): Promise<string>;
export type BufferedTransformOptions = {
    /**
     * Flush synchronously once the buffer reaches this many bytes.
     */
    readonly maxBufferByteLength?: number;
};
export declare function createBufferedTransformStream(options?: BufferedTransformOptions): TransformStream<Uint8Array, Uint8Array>;
export declare function renderToInitialFizzStream({ ReactDOMServer, element, streamOptions, }: {
    ReactDOMServer: {
        renderToReadableStream: typeof import('react-dom/server').renderToReadableStream;
    };
    element: React.ReactElement;
    streamOptions?: Parameters<typeof ReactDOMServer.renderToReadableStream>[1];
}): Promise<ReactDOMServerReadableStream>;
export declare function createRootLayoutValidatorStream(): TransformStream<Uint8Array, Uint8Array>;
export type ContinueStreamOptions = {
    inlinedDataStream: ReadableStream<Uint8Array> | undefined;
    isStaticGeneration: boolean;
    isBuildTimePrerendering: boolean;
    buildId: string;
    getServerInsertedHTML: () => Promise<string>;
    getServerInsertedMetadata: () => Promise<string>;
    validateRootLayout?: boolean;
    /**
     * Suffix to inject after the buffered data, but before the close tags.
     */
    suffix?: string | undefined;
};
export declare function continueFizzStream(renderStream: ReactDOMServerReadableStream, { suffix, inlinedDataStream, isStaticGeneration, isBuildTimePrerendering, buildId, getServerInsertedHTML, getServerInsertedMetadata, validateRootLayout, }: ContinueStreamOptions): Promise<ReadableStream<Uint8Array>>;
type ContinueDynamicPrerenderOptions = {
    getServerInsertedHTML: () => Promise<string>;
    getServerInsertedMetadata: () => Promise<string>;
};
export declare function continueDynamicPrerender(prerenderStream: ReadableStream<Uint8Array>, { getServerInsertedHTML, getServerInsertedMetadata, }: ContinueDynamicPrerenderOptions): Promise<ReadableStream<Uint8Array<ArrayBufferLike>>>;
type ContinueStaticPrerenderOptions = {
    inlinedDataStream: ReadableStream<Uint8Array>;
    getServerInsertedHTML: () => Promise<string>;
    getServerInsertedMetadata: () => Promise<string>;
    isBuildTimePrerendering: boolean;
    buildId: string;
};
export declare function continueStaticPrerender(prerenderStream: ReadableStream<Uint8Array>, { inlinedDataStream, getServerInsertedHTML, getServerInsertedMetadata, isBuildTimePrerendering, buildId, }: ContinueStaticPrerenderOptions): Promise<ReadableStream<Uint8Array<ArrayBufferLike>>>;
export declare function continueStaticFallbackPrerender(prerenderStream: ReadableStream<Uint8Array>, { inlinedDataStream, getServerInsertedHTML, getServerInsertedMetadata, isBuildTimePrerendering, buildId, }: ContinueStaticPrerenderOptions): Promise<ReadableStream<Uint8Array<ArrayBufferLike>>>;
type ContinueResumeOptions = {
    inlinedDataStream: ReadableStream<Uint8Array>;
    getServerInsertedHTML: () => Promise<string>;
    getServerInsertedMetadata: () => Promise<string>;
    delayDataUntilFirstHtmlChunk: boolean;
};
export declare function continueDynamicHTMLResume(renderStream: ReadableStream<Uint8Array>, { delayDataUntilFirstHtmlChunk, inlinedDataStream, getServerInsertedHTML, getServerInsertedMetadata, }: ContinueResumeOptions): Promise<ReadableStream<Uint8Array<ArrayBufferLike>>>;
export declare function createDocumentClosingStream(): ReadableStream<Uint8Array>;
export {};
