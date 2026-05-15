import { CacheSignal } from '../cache-signal';
export declare function trackPendingChunkLoad(promise: Promise<unknown>): void;
export declare function trackPendingImport(exportsOrPromise: unknown): void;
/**
 * A top-level dynamic import (or chunk load):
 *
 *   1. delays a prerender (potentially for a task or longer)
 *   2. may reveal more caches that need be filled
 *
 * So if we see one, we want to extend the duration of `cacheSignal` at least until the import/chunk-load is done.
 */
export declare function trackPendingModules(cacheSignal: CacheSignal): void;
