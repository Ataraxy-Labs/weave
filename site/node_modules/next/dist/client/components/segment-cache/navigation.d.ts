import type { CacheNodeSeedData, FlightRouterState, FlightSegmentPath } from '../../../shared/lib/app-router-types';
import type { CacheNode } from '../../../shared/lib/app-router-types';
import type { HeadData } from '../../../shared/lib/app-router-types';
import type { NormalizedFlightData } from '../../flight-data-helpers';
import { FreshnessPolicy } from '../router-reducer/ppr-navigations';
import { NavigationResultTag } from './types';
type MPANavigationResult = {
    tag: NavigationResultTag.MPA;
    data: string;
};
type SuccessfulNavigationResult = {
    tag: NavigationResultTag.Success;
    data: {
        flightRouterState: FlightRouterState;
        cacheNode: CacheNode;
        canonicalUrl: string;
        renderedSearch: string;
        scrollableSegments: Array<FlightSegmentPath> | null;
        shouldScroll: boolean;
        hash: string;
    };
};
type AsyncNavigationResult = {
    tag: NavigationResultTag.Async;
    data: Promise<MPANavigationResult | SuccessfulNavigationResult>;
};
export type NavigationResult = MPANavigationResult | SuccessfulNavigationResult | AsyncNavigationResult;
/**
 * Navigate to a new URL, using the Segment Cache to construct a response.
 *
 * To allow for synchronous navigations whenever possible, this is not an async
 * function. It returns a promise only if there's no matching prefetch in
 * the cache. Otherwise it returns an immediate result and uses Suspense/RSC to
 * stream in any missing data.
 */
export declare function navigate(url: URL, currentUrl: URL, currentCacheNode: CacheNode | null, currentFlightRouterState: FlightRouterState, nextUrl: string | null, freshnessPolicy: FreshnessPolicy, shouldScroll: boolean, accumulation: {
    collectedDebugInfo?: Array<unknown>;
}): NavigationResult;
export declare function navigateToSeededRoute(now: number, url: URL, canonicalUrl: string, navigationSeed: NavigationSeed, currentUrl: URL, currentCacheNode: CacheNode | null, currentFlightRouterState: FlightRouterState, freshnessPolicy: FreshnessPolicy, nextUrl: string | null, shouldScroll: boolean): SuccessfulNavigationResult | MPANavigationResult;
export type NavigationSeed = {
    tree: FlightRouterState;
    renderedSearch: string;
    data: CacheNodeSeedData | null;
    head: HeadData | null;
};
export declare function convertServerPatchToFullTree(currentTree: FlightRouterState, flightData: Array<NormalizedFlightData>, renderedSearch: string): NavigationSeed;
export {};
