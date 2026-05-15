import type { MetadataContext } from '../types/resolvers';
export type MetadataBaseURL = URL | null;
declare function isStringOrURL(icon: any): icon is string | URL;
/**
 * Given an optional user-provided metadataBase, this determines what the metadataBase should
 * fallback to. Specifically:
 * - In dev, it should always be localhost
 * - In Vercel preview builds, it should be the preview build ID
 * - In start, it should be the user-provided metadataBase value. Otherwise,
 * it'll fall back to the Vercel production deployment, and localhost as a last resort.
 */
export declare function getSocialImageMetadataBaseFallback(metadataBase: MetadataBaseURL): URL;
declare function resolveUrl(url: null | undefined, metadataBase: MetadataBaseURL): null;
declare function resolveUrl(url: string | URL, metadataBase: MetadataBaseURL): URL;
declare function resolveUrl(url: string | MetadataBaseURL | undefined, metadataBase: MetadataBaseURL): MetadataBaseURL;
declare function resolveRelativeUrl(url: string | URL, pathname: string): string | URL;
declare function resolveAbsoluteUrlWithPathname(url: string | URL, metadataBase: MetadataBaseURL, pathname: string, { trailingSlash }: MetadataContext): string;
export { isStringOrURL, resolveUrl, resolveRelativeUrl, resolveAbsoluteUrlWithPathname, };
