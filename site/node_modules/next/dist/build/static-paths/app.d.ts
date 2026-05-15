import type { Params } from '../../server/request/params';
import type { AppPageModule } from '../../server/route-modules/app-page/module';
import type { AppSegment } from '../segment-config/app/app-segments';
import type { PrerenderedRoute, StaticPathsResult } from './types';
import { FallbackMode } from '../../lib/fallback';
import type { IncrementalCache } from '../../server/lib/incremental-cache';
import type { NextConfigComplete } from '../../server/config-shared';
import type { WorkStore } from '../../server/app-render/work-async-storage.external';
import type { AppRouteModule } from '../../server/route-modules/app-route/module.compiled';
import type { NormalizedAppRoute } from '../../shared/lib/router/routes/app';
/**
 * Filters out duplicate parameters from a list of parameters.
 * This function uses a Map to efficiently store and retrieve unique parameter combinations.
 *
 * @param childrenRouteParams - The keys of the parameters. These should be sorted to ensure consistent key generation.
 * @param routeParams - The list of parameter objects to filter.
 * @returns A new array containing only the unique parameter combinations.
 */
export declare function filterUniqueParams(childrenRouteParams: readonly {
    paramName: string;
}[], routeParams: readonly Params[]): Params[];
/**
 * Generates all unique sub-combinations of Route Parameters from a list of Static Parameters.
 * This function creates all possible prefixes of the Route Parameters, which is
 * useful for generating Static Shells that can serve as Fallback Shells for more specific Route Shells.
 *
 * When Root Parameters are provided, the function ensures that Static Shells only
 * include complete sets of Root Parameters. This prevents generating invalid Static Shells
 * that are missing required Root Parameters.
 *
 * Example with Root Parameters ('lang', 'region') and Route Parameters ('lang', 'region', 'slug'):
 *
 * Given the following Static Parameters:
 * ```
 * [
 *   { lang: 'en', region: 'US', slug: ['home'] },
 *   { lang: 'en', region: 'US', slug: ['about'] },
 *   { lang: 'fr', region: 'CA', slug: ['about'] },
 * ]
 * ```
 *
 * The result will be:
 * ```
 * [
 *   { lang: 'en', region: 'US' },  // Complete Root Parameters
 *   { lang: 'en', region: 'US', slug: ['home'] },
 *   { lang: 'en', region: 'US', slug: ['about'] },
 *   { lang: 'fr', region: 'CA' },  // Complete Root Parameters
 *   { lang: 'fr', region: 'CA', slug: ['about'] },
 * ]
 * ```
 *
 * Note that partial combinations like `{ lang: 'en' }` are NOT generated because
 * they don't include the complete set of Root Parameters.
 *
 * For routes without Root Parameters (e.g., `/[slug]`), all sub-combinations are generated
 * as before.
 *
 * @param childrenRouteParams - The children route params. These should be sorted
 *   to ensure consistent key generation for the internal Map.
 * @param routeParams - The list of Static Parameters to filter.
 * @param rootParamKeys - The keys of the Root Parameters. When provided, ensures Static Shells
 *   include all Root Parameters.
 * @returns A new array containing all unique sub-combinations of Route Parameters.
 */
export declare function generateAllParamCombinations(childrenRouteParams: ReadonlyArray<{
    readonly paramName: string;
}>, routeParams: readonly Params[], rootParamKeys: readonly string[]): Params[];
/**
 * Calculates the fallback mode based on the given parameters.
 *
 * @param dynamicParams - Whether dynamic params are enabled.
 * @param fallbackRootParams - The root params that are part of the fallback.
 * @param baseFallbackMode - The base fallback mode to use.
 * @returns The calculated fallback mode.
 */
export declare function calculateFallbackMode(dynamicParams: boolean, fallbackRootParams: readonly string[], baseFallbackMode: FallbackMode | undefined): FallbackMode;
/**
 * Assigns the throwOnEmptyStaticShell property to each of the prerendered routes.
 * This function uses a Trie data structure to efficiently determine whether each route
 * should throw an error when its static shell is empty.
 *
 * A route should not throw on empty static shell if it has child routes in the Trie. For example,
 * if we have two routes, `/blog/first-post` and `/blog/[slug]`, the route for
 * `/blog/[slug]` should not throw because `/blog/first-post` is a more specific concrete route.
 *
 * @param prerenderedRoutes - The prerendered routes.
 * @param pathnameSegments - The keys of the route parameters.
 */
export declare function assignErrorIfEmpty(prerenderedRoutes: readonly PrerenderedRoute[], pathnameSegments: ReadonlyArray<{
    readonly paramName: string;
}>): void;
/**
 * Processes app directory segments to build route parameters from generateStaticParams functions.
 * This function walks through the segments array and calls generateStaticParams for each segment that has it,
 * combining parent parameters with child parameters to build the complete parameter combinations.
 * Uses iterative processing instead of recursion for better performance.
 *
 * @param segments - Array of app directory segments to process
 * @param store - Work store for tracking fetch cache configuration
 * @returns Promise that resolves to an array of all parameter combinations
 */
export declare function generateRouteStaticParams(segments: ReadonlyArray<Readonly<Pick<AppSegment, 'config' | 'generateStaticParams'>>>, store: Pick<WorkStore, 'fetchCache'>, isRoutePPREnabled: boolean): Promise<Params[]>;
/**
 * Processes app directory segments to build route parameters from generateStaticParams functions.
 * This function walks through the segments array and calls generateStaticParams for each segment that has it,
 * combining parent parameters with child parameters to build the complete parameter combinations.
 * Uses iterative processing instead of recursion for better performance.
 *
 * @param segments - Array of app directory segments to process
 * @param store - Work store for tracking fetch cache configuration
 * @returns Promise that resolves to an array of all parameter combinations
 */
export declare function buildAppStaticPaths({ dir, page, route, distDir, cacheComponents, authInterrupts, segments, isrFlushToDisk, cacheHandler, cacheLifeProfiles, requestHeaders, cacheHandlers, cacheMaxMemorySize, fetchCacheKeyPrefix, nextConfigOutput, ComponentMod, isRoutePPREnabled, buildId, rootParamKeys, }: {
    dir: string;
    page: string;
    route: NormalizedAppRoute;
    cacheComponents: boolean;
    authInterrupts: boolean;
    segments: readonly Readonly<AppSegment>[];
    distDir: string;
    isrFlushToDisk?: boolean;
    fetchCacheKeyPrefix?: string;
    cacheHandler?: string;
    cacheHandlers?: NextConfigComplete['cacheHandlers'];
    cacheLifeProfiles?: {
        [profile: string]: import('../../server/use-cache/cache-life').CacheLife;
    };
    cacheMaxMemorySize: number;
    requestHeaders: IncrementalCache['requestHeaders'];
    nextConfigOutput: 'standalone' | 'export' | undefined;
    ComponentMod: AppPageModule | AppRouteModule;
    isRoutePPREnabled: boolean;
    buildId: string;
    rootParamKeys: readonly string[];
}): Promise<StaticPathsResult>;
