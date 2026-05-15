import type { ParsedUrlQuery } from 'querystring';
import type { Rewrite } from '../../../../lib/load-custom-routes';
import { type ParsedRelativeUrl } from './parse-relative-url';
interface ParsedAs extends Omit<ParsedRelativeUrl, 'slashes'> {
    slashes: boolean | undefined;
}
export default function resolveRewrites(asPath: string, pages: string[], rewrites: {
    beforeFiles: Rewrite[];
    afterFiles: Rewrite[];
    fallback: Rewrite[];
}, query: ParsedUrlQuery, resolveHref: (path: string) => string, locales?: readonly string[]): {
    matchedPage: boolean;
    parsedAs: ParsedAs;
    asPath: string;
    resolvedHref?: string;
    externalDest?: boolean;
};
export {};
