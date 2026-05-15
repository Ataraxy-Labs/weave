export interface DebugChannelReadableWriterPair {
    readonly readable: ReadableStream<Uint8Array>;
    readonly writer: WritableStreamDefaultWriter<Uint8Array>;
}
export declare function getOrCreateDebugChannelReadableWriterPair(requestId: string): DebugChannelReadableWriterPair;
export declare function createDebugChannel(requestHeaders: Record<string, string> | undefined): {
    writable?: WritableStream;
    readable?: ReadableStream;
};
