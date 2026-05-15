/**
 * This is a utility function to make scheduling sequential tasks that run back to back easier.
 * We schedule on the same queue (setTimeout) at the same time to ensure no other events can sneak in between.
 */
export declare function prerenderAndAbortInSequentialTasks<R>(prerender: () => Promise<R>, abort: () => void): Promise<R>;
/**
 * Like `prerenderAndAbortInSequentialTasks`, but with another task between `prerender` and `abort`,
 * which allows us to move a part of the render into a separate task.
 */
export declare function prerenderAndAbortInSequentialTasksWithStages<R>(prerender: () => Promise<R>, advanceStage: () => void, abort: () => void): Promise<R>;
export declare class ReactServerResult {
    private _stream;
    constructor(stream: ReadableStream<Uint8Array>);
    tee(): ReadableStream<Uint8Array<ArrayBufferLike>>;
    consume(): ReadableStream<Uint8Array<ArrayBufferLike>>;
}
export type ReactServerPrerenderResolveToType = {
    prelude: ReadableStream<Uint8Array>;
};
export declare function createReactServerPrerenderResult(underlying: Promise<ReactServerPrerenderResolveToType>): Promise<ReactServerPrerenderResult>;
export declare function createReactServerPrerenderResultFromRender(underlying: ReadableStream<Uint8Array>): Promise<ReactServerPrerenderResult>;
export declare class ReactServerPrerenderResult {
    private _chunks;
    private assertChunks;
    private consumeChunks;
    consume(): void;
    constructor(chunks: Array<Uint8Array>);
    asUnclosingStream(): ReadableStream<Uint8Array>;
    consumeAsUnclosingStream(): ReadableStream<Uint8Array>;
    asStream(): ReadableStream<Uint8Array>;
    consumeAsStream(): ReadableStream<Uint8Array>;
}
export declare function processPrelude(unprocessedPrelude: ReadableStream<Uint8Array>): Promise<{
    prelude: ReadableStream<Uint8Array<ArrayBufferLike>>;
    preludeIsEmpty: boolean;
}>;
