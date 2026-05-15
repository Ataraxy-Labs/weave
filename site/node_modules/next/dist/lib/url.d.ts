import type { UrlWithParsedQuery } from 'url';
export declare function isFullStringUrl(url: string): boolean;
export declare function parseUrl(url: string): URL | undefined;
export declare function parseReqUrl(url: string): UrlWithParsedQuery | undefined;
export declare function stripNextRscUnionQuery(relativeUrl: string): string;
