import type { RequestHeaders } from './fetch-server-response';
/**
 * Mutates the provided URL by adding a cache-busting search parameter for CDNs that don't
 * support custom headers. This helps avoid caching conflicts by making each request unique.
 *
 * Rather than relying on the Vary header which some CDNs ignore, we append a search param
 * to create a unique URL that forces a fresh request.
 *
 * Example:
 * URL before: https://example.com/path?query=1
 * URL after: https://example.com/path?query=1&_rsc=abc123
 *
 * Note: This function mutates the input URL directly and does not return anything.
 *
 * TODO: Since we need to use a search param anyway, we could simplify by removing the custom
 * headers approach entirely and just use search params.
 */
export declare const setCacheBustingSearchParam: (url: URL, headers: RequestHeaders) => void;
/**
 * Sets a cache-busting search parameter on a URL using a provided hash value.
 *
 * This function performs the same logic as `setCacheBustingSearchParam` but accepts
 * a pre-computed hash instead of computing it from headers.
 *
 * Example:
 * URL before: https://example.com/path?query=1
 * hash: "abc123"
 * URL after: https://example.com/path?query=1&_rsc=abc123
 *
 * If the hash is null, we will set `_rsc` search param without a value.
 * Like this: https://example.com/path?query=1&_rsc
 *
 * Note: This function mutates the input URL directly and does not return anything.
 */
export declare const setCacheBustingSearchParamWithHash: (url: URL, hash: string) => void;
