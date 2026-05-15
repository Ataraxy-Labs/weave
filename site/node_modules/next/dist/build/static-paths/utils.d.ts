import type { LoaderTree } from '../../server/lib/app-dir-module';
import type { Params } from '../../server/request/params';
import type { AppPageRouteModule } from '../../server/route-modules/app-page/module.compiled';
import type { AppRouteRouteModule } from '../../server/route-modules/app-route/module.compiled';
import type { DynamicParamTypes } from '../../shared/lib/app-router-types';
import { type NormalizedAppRoute } from '../../shared/lib/router/routes/app';
import type { AppSegment } from '../segment-config/app/app-segments';
import type { FallbackRouteParam } from './types';
/**
 * Encodes a parameter value using the provided encoder.
 *
 * @param value - The value to encode.
 * @param encoder - The encoder to use.
 * @returns The encoded value.
 */
export declare function encodeParam(value: string | string[], encoder: (value: string) => string): string;
/**
 * Normalizes a pathname to a consistent format.
 *
 * @param pathname - The pathname to normalize.
 * @returns The normalized pathname.
 */
export declare function normalizePathname(pathname: string): string;
/**
 * Extracts segments that contribute to the pathname by traversing the loader tree
 * based on the route module type.
 *
 * @param routeModule - The app route module (page or route handler)
 * @param segments - Array of AppSegment objects collected from the route
 * @param page - The target pathname to match against, INCLUDING interception
 *               markers (e.g., "/blog/[slug]", "/(.)photo/[id]")
 * @returns Array of segments with param info that contribute to the pathname
 */
export declare function extractPathnameRouteParamSegments(routeModule: AppRouteRouteModule | AppPageRouteModule, segments: readonly Readonly<AppSegment>[], route: NormalizedAppRoute): Array<{
    readonly name: string;
    readonly paramName: string;
    readonly paramType: DynamicParamTypes;
}>;
export declare function extractPathnameRouteParamSegmentsFromSegments(segments: readonly Readonly<AppSegment>[]): Array<{
    readonly name: string;
    readonly paramName: string;
    readonly paramType: DynamicParamTypes;
}>;
/**
 * Resolves all route parameters from the loader tree. This function uses
 * tree-based traversal to correctly handle the hierarchical structure of routes
 * and accurately determine parameter values based on their depth in the tree.
 *
 * This processes both regular route parameters (from the main children route) and
 * parallel route parameters (from slots like @modal, @sidebar).
 *
 * Unlike interpolateParallelRouteParams (which has a complete URL at runtime),
 * this build-time function determines which route params are unknown.
 * The pathname may contain placeholders like [slug], making it incomplete.
 *
 * @param loaderTree - The loader tree structure containing route hierarchy
 * @param params - The current route parameters object (will be mutated)
 * @param route - The current route being processed
 * @param fallbackRouteParams - Array of fallback route parameters (will be mutated)
 */
export declare function resolveRouteParamsFromTree(loaderTree: LoaderTree, params: Params, route: NormalizedAppRoute, fallbackRouteParams: FallbackRouteParam[]): void;
