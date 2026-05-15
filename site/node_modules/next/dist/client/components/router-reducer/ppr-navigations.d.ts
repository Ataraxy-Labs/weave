import type { CacheNodeSeedData, FlightRouterState, FlightSegmentPath } from '../../../shared/lib/app-router-types';
import type { CacheNode } from '../../../shared/lib/app-router-types';
import type { HeadData } from '../../../shared/lib/app-router-types';
export type NavigationTask = {
    status: NavigationTaskStatus;
    route: FlightRouterState;
    node: CacheNode;
    dynamicRequestTree: FlightRouterState | null;
    refreshUrl: string | null;
    children: Map<string, NavigationTask> | null;
};
export declare const enum FreshnessPolicy {
    Default = 0,
    Hydration = 1,
    HistoryTraversal = 2,
    RefreshAll = 3,
    HMRRefresh = 4
}
declare const enum NavigationTaskStatus {
    Pending = 0,
    Fulfilled = 1,
    Rejected = 2
}
export type NavigationRequestAccumulation = {
    scrollableSegments: Array<FlightSegmentPath> | null;
    separateRefreshUrls: Set<string> | null;
};
export declare function createInitialCacheNodeForHydration(navigatedAt: number, initialTree: FlightRouterState, seedData: CacheNodeSeedData | null, seedHead: HeadData): CacheNode;
export declare function startPPRNavigation(navigatedAt: number, oldUrl: URL, oldCacheNode: CacheNode | null, oldRouterState: FlightRouterState, newRouterState: FlightRouterState, freshness: FreshnessPolicy, seedData: CacheNodeSeedData | null, seedHead: HeadData | null, prefetchData: CacheNodeSeedData | null, prefetchHead: HeadData | null, isPrefetchHeadPartial: boolean, isSamePageNavigation: boolean, accumulation: NavigationRequestAccumulation): NavigationTask | null;
export declare function spawnDynamicRequests(task: NavigationTask, primaryUrl: URL, nextUrl: string | null, freshnessPolicy: FreshnessPolicy, accumulation: NavigationRequestAccumulation): void;
type PendingDeferredRsc<T> = Promise<T> & {
    status: 'pending';
    resolve: (value: T, debugInfo: Array<any> | null) => void;
    reject: (error: any, debugInfo: Array<any> | null) => void;
    tag: Symbol;
    _debugInfo: Array<any>;
};
type FulfilledDeferredRsc<T> = Promise<T> & {
    status: 'fulfilled';
    value: T;
    resolve: (value: T, debugInfo: Array<any> | null) => void;
    reject: (error: any, debugInfo: Array<any> | null) => void;
    tag: Symbol;
    _debugInfo: Array<any>;
};
type RejectedDeferredRsc<T> = Promise<T> & {
    status: 'rejected';
    reason: any;
    resolve: (value: T, debugInfo: Array<any> | null) => void;
    reject: (error: any, debugInfo: Array<any> | null) => void;
    tag: Symbol;
    _debugInfo: Array<any>;
};
type DeferredRsc<T extends React.ReactNode = React.ReactNode> = PendingDeferredRsc<T> | FulfilledDeferredRsc<T> | RejectedDeferredRsc<T>;
export declare function isDeferredRsc(value: any): value is DeferredRsc;
export {};
