import type { NonStaticRenderStage } from './app-render/staged-rendering';
import type { RequestStore } from './app-render/work-unit-async-storage.external';
export declare function isHangingPromiseRejectionError(err: unknown): err is HangingPromiseRejectionError;
declare class HangingPromiseRejectionError extends Error {
    readonly route: string;
    readonly expression: string;
    readonly digest = "HANGING_PROMISE_REJECTION";
    constructor(route: string, expression: string);
}
export declare function makeDevtoolsIOAwarePromise<T>(underlying: T, requestStore: RequestStore, stage: NonStaticRenderStage): Promise<T>;
export {};
