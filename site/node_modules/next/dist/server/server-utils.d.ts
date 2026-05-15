import type { Rewrite } from '../lib/load-custom-routes';
import type { RouteMatchFn } from '../shared/lib/router/utils/route-matcher';
import type { NextConfig } from './config';
import type { BaseNextRequest } from './base-http';
import type { ParsedUrlQuery } from 'querystring';
import type { UrlWithParsedQuery } from 'url';
import { getNamedRouteRegex } from '../shared/lib/router/utils/route-regex';
import type { IncomingHttpHeaders, IncomingMessage } from 'http';
import type { DeepReadonly } from '../shared/lib/deep-readonly';
export declare function normalizeCdnUrl(req: BaseNextRequest | IncomingMessage, paramKeys: string[]): string | undefined;
export declare function interpolateDynamicPath(pathname: string, params: ParsedUrlQuery, defaultRouteRegex?: ReturnType<typeof getNamedRouteRegex> | undefined): string;
export declare function normalizeDynamicRouteParams(query: ParsedUrlQuery, defaultRouteRegex: ReturnType<typeof getNamedRouteRegex>, defaultRouteMatches: ParsedUrlQuery, ignoreMissingOptional: boolean): {
    params: ParsedUrlQuery;
    hasValidParams: boolean;
};
export declare function getServerUtils({ page, i18n, basePath, rewrites, pageIsDynamic, trailingSlash, caseSensitive, }: {
    page: string;
    i18n?: NextConfig['i18n'];
    basePath: string;
    rewrites: DeepReadonly<{
        fallback?: ReadonlyArray<Rewrite>;
        afterFiles?: ReadonlyArray<Rewrite>;
        beforeFiles?: ReadonlyArray<Rewrite>;
    }>;
    pageIsDynamic: boolean;
    trailingSlash?: boolean;
    caseSensitive: boolean;
}): {
    handleRewrites: (req: BaseNextRequest | IncomingMessage, parsedUrl: DeepReadonly<UrlWithParsedQuery>) => {
        rewriteParams: Record<string, string>;
        rewrittenParsedUrl: UrlWithParsedQuery;
    };
    defaultRouteRegex: {
        namedRegex: string;
        routeKeys: {
            [named: string]: string;
        };
        pathToRegexpPattern: string;
        reference: import("../shared/lib/router/utils/route-regex").RegexReference;
        groups: {
            [groupName: string]: import("../shared/lib/router/utils/route-regex").Group;
        };
        re: RegExp;
    } | undefined;
    dynamicRouteMatcher: RouteMatchFn | undefined;
    defaultRouteMatches: ParsedUrlQuery | undefined;
    normalizeQueryParams: (query: Record<string, string | string[] | undefined>, routeParamKeys: Set<string>) => void;
    getParamsFromRouteMatches: (routeMatchesHeader: string) => import("./request/params").Params | null;
    /**
     * Normalize dynamic route params.
     *
     * @param query - The query params to normalize.
     * @param ignoreMissingOptional - Whether to ignore missing optional params.
     * @returns The normalized params and whether they are valid.
     */
    normalizeDynamicRouteParams: (query: ParsedUrlQuery, ignoreMissingOptional: boolean) => {
        params: ParsedUrlQuery;
        hasValidParams: boolean;
    };
    normalizeCdnUrl: (req: BaseNextRequest | IncomingMessage, paramKeys: string[]) => string | undefined;
    interpolateDynamicPath: (pathname: string, params: Record<string, undefined | string | string[]>) => string;
    filterInternalQuery: (query: ParsedUrlQuery, paramKeys: string[]) => void;
};
export declare function getPreviouslyRevalidatedTags(headers: IncomingHttpHeaders, previewModeId: string | undefined): string[];
