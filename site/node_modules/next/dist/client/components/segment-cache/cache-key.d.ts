type Opaque<K, T> = T & {
    __brand: K;
};
export type NormalizedPathname = Opaque<'NormalizedPathname', string>;
export type NormalizedSearch = Opaque<'NormalizedSearch', string>;
export type NormalizedNextUrl = Opaque<'NormalizedNextUrl', string>;
export type RouteCacheKey = Opaque<'RouteCacheKey', {
    pathname: NormalizedPathname;
    search: NormalizedSearch;
    nextUrl: NormalizedNextUrl | null;
}>;
export declare function createCacheKey(originalHref: string, nextUrl: string | null): RouteCacheKey;
export {};
