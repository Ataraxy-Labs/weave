import { FetchStrategy } from './types';
import type { NormalizedPathname, NormalizedSearch, NormalizedNextUrl } from './cache-key';
import type { RouteTree } from './cache';
import { type FallbackType } from './cache-map';
type Opaque<T, K> = T & {
    __brand: K;
};
/**
 * A linked-list of all the params (or other param-like) inputs that a cache
 * entry may vary by. This is used by the CacheMap module to reuse cache entries
 * across different param values. If a param has a value of Fallback, it means
 * the cache entry is reusable for all possible values of that param. See
 * cache-map.ts for details.
 *
 * A segment's vary path is a pure function of a segment's position in a
 * particular route tree and the (post-rewrite) URL that is being queried. More
 * concretely, successive queries of the cache for the same segment always use
 * the same vary path.
 *
 * A route's vary path is simpler: it's comprised of the pathname, search
 * string, and Next-URL header.
 */
export type VaryPath = {
    value: string | null | FallbackType;
    parent: VaryPath | null;
};
export type RouteVaryPath = Opaque<{
    value: NormalizedPathname;
    parent: {
        value: NormalizedSearch;
        parent: {
            value: NormalizedNextUrl | null | FallbackType;
            parent: null;
        };
    };
}, 'RouteVaryPath'>;
export type LayoutVaryPath = Opaque<{
    value: string;
    parent: PartialSegmentVaryPath | null;
}, 'LayoutVaryPath'>;
export type PageVaryPath = Opaque<{
    value: string;
    parent: {
        value: NormalizedSearch | FallbackType;
        parent: PartialSegmentVaryPath | null;
    };
}, 'PageVaryPath'>;
export type SegmentVaryPath = LayoutVaryPath | PageVaryPath;
export type PartialSegmentVaryPath = Opaque<VaryPath, 'PartialSegmentVaryPath'>;
export declare function getRouteVaryPath(pathname: NormalizedPathname, search: NormalizedSearch, nextUrl: NormalizedNextUrl | null): RouteVaryPath;
export declare function getFulfilledRouteVaryPath(pathname: NormalizedPathname, search: NormalizedSearch, nextUrl: NormalizedNextUrl | null, couldBeIntercepted: boolean): RouteVaryPath;
export declare function appendLayoutVaryPath(parentPath: PartialSegmentVaryPath | null, cacheKey: string): PartialSegmentVaryPath;
export declare function finalizeLayoutVaryPath(requestKey: string, varyPath: PartialSegmentVaryPath | null): LayoutVaryPath;
export declare function finalizePageVaryPath(requestKey: string, renderedSearch: NormalizedSearch, varyPath: PartialSegmentVaryPath | null): PageVaryPath;
export declare function finalizeMetadataVaryPath(pageRequestKey: string, renderedSearch: NormalizedSearch, varyPath: PartialSegmentVaryPath | null): PageVaryPath;
export declare function getSegmentVaryPathForRequest(fetchStrategy: FetchStrategy, tree: RouteTree): SegmentVaryPath;
export declare function clonePageVaryPathWithNewSearchParams(originalVaryPath: PageVaryPath, newSearch: NormalizedSearch): PageVaryPath;
export {};
