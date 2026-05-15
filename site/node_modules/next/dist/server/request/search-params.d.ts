import type { WorkStore } from '../app-render/work-async-storage.external';
export type SearchParams = {
    [key: string]: string | string[] | undefined;
};
export declare function createSearchParamsFromClient(underlyingSearchParams: SearchParams, workStore: WorkStore): Promise<SearchParams>;
export declare const createServerSearchParamsForMetadata: typeof createServerSearchParamsForServerPage;
export declare function createServerSearchParamsForServerPage(underlyingSearchParams: SearchParams, workStore: WorkStore): Promise<SearchParams>;
export declare function createPrerenderSearchParamsForClientPage(workStore: WorkStore): Promise<SearchParams>;
/**
 * This is a variation of `makeErroringSearchParams` that always throws an
 * error on access, because accessing searchParams inside of `"use cache"` is
 * not allowed.
 */
export declare function makeErroringSearchParamsForUseCache(workStore: WorkStore): Promise<SearchParams>;
