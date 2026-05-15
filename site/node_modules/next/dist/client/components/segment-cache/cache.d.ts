import type { LoadingModuleData } from '../../../shared/lib/app-router-types';
import type { Segment as FlightRouterStateSegment } from '../../../shared/lib/app-router-types';
import { HasLoadingBoundary } from '../../../shared/lib/app-router-types';
import { type PrefetchTask, type PrefetchSubtaskResult } from './scheduler';
import { type SegmentVaryPath, type PageVaryPath } from './vary-path';
import type { NormalizedSearch, RouteCacheKey } from './cache-key';
import { type UnknownMapEntry } from './cache-map';
import { type SegmentRequestKey } from '../../../shared/lib/segment-cache/segment-value-encoding';
import type { FlightRouterState } from '../../../shared/lib/app-router-types';
import { FetchStrategy } from './types';
/**
 * Ensures a minimum stale time of 30s to avoid issues where the server sends a too
 * short-lived stale time, which would prevent anything from being prefetched.
 */
export declare function getStaleTimeMs(staleTimeSeconds: number): number;
type RouteTreeShared = {
    requestKey: SegmentRequestKey;
    segment: FlightRouterStateSegment;
    slots: null | {
        [parallelRouteKey: string]: RouteTree;
    };
    isRootLayout: boolean;
    hasLoadingBoundary: HasLoadingBoundary;
    hasRuntimePrefetch: boolean;
};
type LayoutRouteTree = RouteTreeShared & {
    isPage: false;
    varyPath: SegmentVaryPath;
};
type PageRouteTree = RouteTreeShared & {
    isPage: true;
    varyPath: PageVaryPath;
};
export type RouteTree = LayoutRouteTree | PageRouteTree;
type RouteCacheEntryShared = {
    couldBeIntercepted: boolean;
    ref: UnknownMapEntry | null;
    size: number;
    staleAt: number;
    version: number;
};
/**
 * Tracks the status of a cache entry as it progresses from no data (Empty),
 * waiting for server data (Pending), and finished (either Fulfilled or
 * Rejected depending on the response from the server.
 */
export declare const enum EntryStatus {
    Empty = 0,
    Pending = 1,
    Fulfilled = 2,
    Rejected = 3
}
type PendingRouteCacheEntry = RouteCacheEntryShared & {
    status: EntryStatus.Empty | EntryStatus.Pending;
    blockedTasks: Set<PrefetchTask> | null;
    canonicalUrl: null;
    renderedSearch: null;
    tree: null;
    metadata: null;
    isPPREnabled: false;
};
type RejectedRouteCacheEntry = RouteCacheEntryShared & {
    status: EntryStatus.Rejected;
    blockedTasks: Set<PrefetchTask> | null;
    canonicalUrl: null;
    renderedSearch: null;
    tree: null;
    metadata: null;
    isPPREnabled: boolean;
};
export type FulfilledRouteCacheEntry = RouteCacheEntryShared & {
    status: EntryStatus.Fulfilled;
    blockedTasks: null;
    canonicalUrl: string;
    renderedSearch: NormalizedSearch;
    tree: RouteTree;
    metadata: RouteTree;
    isPPREnabled: boolean;
};
export type RouteCacheEntry = PendingRouteCacheEntry | FulfilledRouteCacheEntry | RejectedRouteCacheEntry;
type SegmentCacheEntryShared = {
    fetchStrategy: FetchStrategy;
    ref: UnknownMapEntry | null;
    size: number;
    staleAt: number;
    version: number;
};
export type EmptySegmentCacheEntry = SegmentCacheEntryShared & {
    status: EntryStatus.Empty;
    rsc: null;
    loading: null;
    isPartial: true;
    promise: null;
};
export type PendingSegmentCacheEntry = SegmentCacheEntryShared & {
    status: EntryStatus.Pending;
    rsc: null;
    loading: null;
    isPartial: boolean;
    promise: null | PromiseWithResolvers<FulfilledSegmentCacheEntry | null>;
};
type RejectedSegmentCacheEntry = SegmentCacheEntryShared & {
    status: EntryStatus.Rejected;
    rsc: null;
    loading: null;
    isPartial: true;
    promise: null;
};
export type FulfilledSegmentCacheEntry = SegmentCacheEntryShared & {
    status: EntryStatus.Fulfilled;
    rsc: React.ReactNode | null;
    loading: LoadingModuleData | Promise<LoadingModuleData>;
    isPartial: boolean;
    promise: null;
};
export type SegmentCacheEntry = EmptySegmentCacheEntry | PendingSegmentCacheEntry | RejectedSegmentCacheEntry | FulfilledSegmentCacheEntry;
export type NonEmptySegmentCacheEntry = Exclude<SegmentCacheEntry, EmptySegmentCacheEntry>;
export declare function getCurrentCacheVersion(): number;
/**
 * Used to clear the client prefetch cache when a server action calls
 * revalidatePath or revalidateTag. Eventually we will support only clearing the
 * segments that were actually affected, but there's more work to be done on the
 * server before the client is able to do this correctly.
 */
export declare function revalidateEntireCache(nextUrl: string | null, tree: FlightRouterState): void;
export declare function pingInvalidationListeners(nextUrl: string | null, tree: FlightRouterState): void;
export declare function readRouteCacheEntry(now: number, key: RouteCacheKey): RouteCacheEntry | null;
export declare function readSegmentCacheEntry(now: number, varyPath: SegmentVaryPath): SegmentCacheEntry | null;
export declare function waitForSegmentCacheEntry(pendingEntry: PendingSegmentCacheEntry): Promise<FulfilledSegmentCacheEntry | null>;
/**
 * Checks if an entry for a route exists in the cache. If so, it returns the
 * entry, If not, it adds an empty entry to the cache and returns it.
 */
export declare function readOrCreateRouteCacheEntry(now: number, task: PrefetchTask, key: RouteCacheKey): RouteCacheEntry;
export declare function requestOptimisticRouteCacheEntry(now: number, requestedUrl: URL, nextUrl: string | null): FulfilledRouteCacheEntry | null;
/**
 * Checks if an entry for a segment exists in the cache. If so, it returns the
 * entry, If not, it adds an empty entry to the cache and returns it.
 */
export declare function readOrCreateSegmentCacheEntry(now: number, fetchStrategy: FetchStrategy, route: FulfilledRouteCacheEntry, tree: RouteTree): SegmentCacheEntry;
export declare function readOrCreateRevalidatingSegmentEntry(now: number, fetchStrategy: FetchStrategy, route: FulfilledRouteCacheEntry, tree: RouteTree): SegmentCacheEntry;
export declare function overwriteRevalidatingSegmentCacheEntry(fetchStrategy: FetchStrategy, route: FulfilledRouteCacheEntry, tree: RouteTree): EmptySegmentCacheEntry;
export declare function upsertSegmentEntry(now: number, varyPath: SegmentVaryPath, candidateEntry: SegmentCacheEntry): SegmentCacheEntry | null;
export declare function createDetachedSegmentCacheEntry(staleAt: number): EmptySegmentCacheEntry;
export declare function upgradeToPendingSegment(emptyEntry: EmptySegmentCacheEntry, fetchStrategy: FetchStrategy): PendingSegmentCacheEntry;
export declare function convertRouteTreeToFlightRouterState(routeTree: RouteTree): FlightRouterState;
export declare function fetchRouteOnCacheMiss(entry: PendingRouteCacheEntry, task: PrefetchTask, key: RouteCacheKey): Promise<PrefetchSubtaskResult<null> | null>;
export declare function fetchSegmentOnCacheMiss(route: FulfilledRouteCacheEntry, segmentCacheEntry: PendingSegmentCacheEntry, routeKey: RouteCacheKey, tree: RouteTree): Promise<PrefetchSubtaskResult<FulfilledSegmentCacheEntry> | null>;
export declare function fetchSegmentPrefetchesUsingDynamicRequest(task: PrefetchTask, route: FulfilledRouteCacheEntry, fetchStrategy: FetchStrategy.LoadingBoundary | FetchStrategy.PPRRuntime | FetchStrategy.Full, dynamicRequestTree: FlightRouterState, spawnedEntries: Map<SegmentRequestKey, PendingSegmentCacheEntry>): Promise<PrefetchSubtaskResult<null> | null>;
/**
 * Checks whether the new fetch strategy is likely to provide more content than the old one.
 *
 * Generally, when an app uses dynamic data, a "more specific" fetch strategy is expected to provide more content:
 * - `LoadingBoundary` only provides static layouts
 * - `PPR` can provide shells for each segment (even for segments that use dynamic data)
 * - `PPRRuntime` can additionally include content that uses searchParams, params, or cookies
 * - `Full` includes all the content, even if it uses dynamic data
 *
 * However, it's possible that a more specific fetch strategy *won't* give us more content if:
 * - a segment is fully static
 *   (then, `PPR`/`PPRRuntime`/`Full` will all yield equivalent results)
 * - providing searchParams/params/cookies doesn't reveal any more content, e.g. because of an `await connection()`
 *   (then, `PPR` and `PPRRuntime` will yield equivalent results, only `Full` will give us more)
 * Because of this, when comparing two segments, we should also check if the existing segment is partial.
 * If it's not partial, then there's no need to prefetch it again, even using a "more specific" strategy.
 * There's currently no way to know if `PPRRuntime` will yield more data that `PPR`, so we have to assume it will.
 *
 * Also note that, in practice, we don't expect to be comparing `LoadingBoundary` to `PPR`/`PPRRuntime`,
 * because a non-PPR-enabled route wouldn't ever use the latter strategies. It might however use `Full`.
 */
export declare function canNewFetchStrategyProvideMoreContent(currentStrategy: FetchStrategy, newStrategy: FetchStrategy): boolean;
export {};
