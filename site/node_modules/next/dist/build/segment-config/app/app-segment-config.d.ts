export type Prefetch = StaticPrefetch | RuntimePrefetch;
export type PrefetchForTypeCheckInternal = __GenericPrefetch | Prefetch;
interface __GenericPrefetch {
    mode: string;
    samples?: Array<WideRuntimeSample>;
    from?: string[];
    expectUnableToVerify?: boolean;
}
interface StaticPrefetch {
    mode: 'static';
    from?: string[];
    expectUnableToVerify?: boolean;
}
interface RuntimePrefetch {
    mode: 'runtime';
    samples: Array<RuntimeSample>;
    from?: string[];
    expectUnableToVerify?: boolean;
}
type WideRuntimeSample = {
    cookies?: RuntimeSample['cookies'];
    headers?: Array<string[]>;
    params?: RuntimeSample['params'];
    searchParams?: RuntimeSample['searchParams'];
};
type RuntimeSample = {
    cookies?: Array<{
        name: string;
        value: string;
        httpOnly?: boolean;
        path?: string;
    }>;
    headers?: Array<[string, string]>;
    params?: {
        [key: string]: string | string[];
    };
    searchParams?: {
        [key: string]: string | string[] | undefined;
    };
};
/**
 * Parse the app segment config.
 * @param data - The data to parse.
 * @param route - The route of the app.
 * @returns The parsed app segment config.
 */
export declare function parseAppSegmentConfig(data: unknown, route: string): AppSegmentConfig;
/**
 * The configuration for a page.
 */
export type AppSegmentConfig = {
    /**
     * The revalidation period for the page in seconds, or false to disable ISR.
     */
    revalidate?: number | false;
    /**
     * Whether the page supports dynamic parameters.
     */
    dynamicParams?: boolean;
    /**
     * The dynamic behavior of the page.
     */
    dynamic?: 'auto' | 'error' | 'force-static' | 'force-dynamic';
    /**
     * The caching behavior of the page.
     */
    fetchCache?: 'auto' | 'default-cache' | 'default-no-store' | 'force-cache' | 'force-no-store' | 'only-cache' | 'only-no-store';
    /**
     * How this segment should be prefetched.
     */
    unstable_prefetch?: Prefetch;
    /**
     * The preferred region for the page.
     */
    preferredRegion?: string | string[];
    /**
     * The runtime to use for the page.
     */
    runtime?: 'edge' | 'nodejs';
    /**
     * The maximum duration for the page in seconds.
     */
    maxDuration?: number;
};
export {};
