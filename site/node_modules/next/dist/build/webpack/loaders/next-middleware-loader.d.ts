import type { ProxyMatcher } from '../../analysis/get-page-static-info';
export type MiddlewareLoaderOptions = {
    absolutePagePath: string;
    page: string;
    rootDir: string;
    matchers?: string;
    preferredRegion: string | string[] | undefined;
    middlewareConfig: string;
};
export declare function encodeMatchers(matchers: ProxyMatcher[]): string;
export declare function decodeMatchers(encodedMatchers: string): ProxyMatcher[];
export default function middlewareLoader(this: any): Promise<string>;
