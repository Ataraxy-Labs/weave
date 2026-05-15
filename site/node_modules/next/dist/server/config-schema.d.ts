import type { NextConfig } from './config';
import { z } from 'next/dist/compiled/zod';
import type zod from 'next/dist/compiled/zod';
import type { SizeLimit } from '../types';
export declare const experimentalSchema: {
    adapterPath: z.ZodOptional<z.ZodString>;
    useSkewCookie: z.ZodOptional<z.ZodBoolean>;
    after: z.ZodOptional<z.ZodBoolean>;
    appNavFailHandling: z.ZodOptional<z.ZodBoolean>;
    preloadEntriesOnStart: z.ZodOptional<z.ZodBoolean>;
    allowedRevalidateHeaderKeys: z.ZodOptional<z.ZodArray<z.ZodString, "many">>;
    staleTimes: z.ZodOptional<z.ZodObject<{
        dynamic: z.ZodOptional<z.ZodNumber>;
        static: z.ZodOptional<z.ZodNumber>;
    }, "strip", z.ZodTypeAny, {
        static?: number | undefined;
        dynamic?: number | undefined;
    }, {
        static?: number | undefined;
        dynamic?: number | undefined;
    }>>;
    cacheLife: z.ZodOptional<z.ZodRecord<z.ZodString, z.ZodObject<{
        stale: z.ZodOptional<z.ZodNumber>;
        revalidate: z.ZodOptional<z.ZodNumber>;
        expire: z.ZodOptional<z.ZodNumber>;
    }, "strip", z.ZodTypeAny, {
        revalidate?: number | undefined;
        expire?: number | undefined;
        stale?: number | undefined;
    }, {
        revalidate?: number | undefined;
        expire?: number | undefined;
        stale?: number | undefined;
    }>>>;
    cacheHandlers: z.ZodOptional<z.ZodRecord<z.ZodString, z.ZodOptional<z.ZodString>>>;
    clientRouterFilter: z.ZodOptional<z.ZodBoolean>;
    clientRouterFilterRedirects: z.ZodOptional<z.ZodBoolean>;
    clientRouterFilterAllowedRate: z.ZodOptional<z.ZodNumber>;
    cpus: z.ZodOptional<z.ZodNumber>;
    memoryBasedWorkersCount: z.ZodOptional<z.ZodBoolean>;
    craCompat: z.ZodOptional<z.ZodBoolean>;
    caseSensitiveRoutes: z.ZodOptional<z.ZodBoolean>;
    clientParamParsingOrigins: z.ZodOptional<z.ZodArray<z.ZodString, "many">>;
    dynamicOnHover: z.ZodOptional<z.ZodBoolean>;
    disableOptimizedLoading: z.ZodOptional<z.ZodBoolean>;
    disablePostcssPresetEnv: z.ZodOptional<z.ZodBoolean>;
    cacheComponents: z.ZodOptional<z.ZodBoolean>;
    inlineCss: z.ZodOptional<z.ZodBoolean>;
    esmExternals: z.ZodOptional<z.ZodUnion<[z.ZodBoolean, z.ZodLiteral<"loose">]>>;
    serverActions: z.ZodOptional<z.ZodObject<{
        bodySizeLimit: z.ZodOptional<z.ZodType<SizeLimit, z.ZodTypeDef, SizeLimit>>;
        allowedOrigins: z.ZodOptional<z.ZodArray<z.ZodString, "many">>;
    }, "strip", z.ZodTypeAny, {
        bodySizeLimit?: SizeLimit | undefined;
        allowedOrigins?: string[] | undefined;
    }, {
        bodySizeLimit?: SizeLimit | undefined;
        allowedOrigins?: string[] | undefined;
    }>>;
    maxPostponedStateSize: z.ZodOptional<z.ZodType<SizeLimit, z.ZodTypeDef, SizeLimit>>;
    extensionAlias: z.ZodOptional<z.ZodRecord<z.ZodString, z.ZodAny>>;
    externalDir: z.ZodOptional<z.ZodBoolean>;
    externalMiddlewareRewritesResolve: z.ZodOptional<z.ZodBoolean>;
    externalProxyRewritesResolve: z.ZodOptional<z.ZodBoolean>;
    fallbackNodePolyfills: z.ZodOptional<z.ZodLiteral<false>>;
    fetchCacheKeyPrefix: z.ZodOptional<z.ZodString>;
    forceSwcTransforms: z.ZodOptional<z.ZodBoolean>;
    fullySpecified: z.ZodOptional<z.ZodBoolean>;
    gzipSize: z.ZodOptional<z.ZodBoolean>;
    imgOptConcurrency: z.ZodNullable<z.ZodOptional<z.ZodNumber>>;
    imgOptTimeoutInSeconds: z.ZodOptional<z.ZodNumber>;
    imgOptMaxInputPixels: z.ZodOptional<z.ZodNumber>;
    imgOptSequentialRead: z.ZodNullable<z.ZodOptional<z.ZodBoolean>>;
    imgOptSkipMetadata: z.ZodNullable<z.ZodOptional<z.ZodBoolean>>;
    isrFlushToDisk: z.ZodOptional<z.ZodBoolean>;
    largePageDataBytes: z.ZodOptional<z.ZodNumber>;
    linkNoTouchStart: z.ZodOptional<z.ZodBoolean>;
    manualClientBasePath: z.ZodOptional<z.ZodBoolean>;
    middlewarePrefetch: z.ZodOptional<z.ZodEnum<["strict", "flexible"]>>;
    proxyPrefetch: z.ZodOptional<z.ZodEnum<["strict", "flexible"]>>;
    middlewareClientMaxBodySize: z.ZodOptional<z.ZodType<SizeLimit, z.ZodTypeDef, SizeLimit>>;
    proxyClientMaxBodySize: z.ZodOptional<z.ZodType<SizeLimit, z.ZodTypeDef, SizeLimit>>;
    multiZoneDraftMode: z.ZodOptional<z.ZodBoolean>;
    cssChunking: z.ZodOptional<z.ZodUnion<[z.ZodBoolean, z.ZodLiteral<"strict">]>>;
    nextScriptWorkers: z.ZodOptional<z.ZodBoolean>;
    optimizeCss: z.ZodOptional<z.ZodUnion<[z.ZodBoolean, z.ZodAny]>>;
    optimisticClientCache: z.ZodOptional<z.ZodBoolean>;
    parallelServerCompiles: z.ZodOptional<z.ZodBoolean>;
    parallelServerBuildTraces: z.ZodOptional<z.ZodBoolean>;
    ppr: z.ZodOptional<z.ZodReadonly<z.ZodUnion<[z.ZodBoolean, z.ZodLiteral<"incremental">]>>>;
    taint: z.ZodOptional<z.ZodBoolean>;
    prerenderEarlyExit: z.ZodOptional<z.ZodBoolean>;
    proxyTimeout: z.ZodOptional<z.ZodNumber>;
    rootParams: z.ZodOptional<z.ZodBoolean>;
    isolatedDevBuild: z.ZodOptional<z.ZodBoolean>;
    mcpServer: z.ZodOptional<z.ZodBoolean>;
    removeUncaughtErrorAndRejectionListeners: z.ZodOptional<z.ZodBoolean>;
    validateRSCRequestHeaders: z.ZodOptional<z.ZodBoolean>;
    scrollRestoration: z.ZodOptional<z.ZodBoolean>;
    sri: z.ZodOptional<z.ZodObject<{
        algorithm: z.ZodOptional<z.ZodEnum<["sha256", "sha384", "sha512"]>>;
    }, "strip", z.ZodTypeAny, {
        algorithm?: "sha256" | "sha384" | "sha512" | undefined;
    }, {
        algorithm?: "sha256" | "sha384" | "sha512" | undefined;
    }>>;
    swcPlugins: z.ZodOptional<z.ZodArray<z.ZodTuple<[z.ZodString, z.ZodRecord<z.ZodString, z.ZodAny>], null>, "many">>;
    swcTraceProfiling: z.ZodOptional<z.ZodBoolean>;
    urlImports: z.ZodOptional<z.ZodAny>;
    viewTransition: z.ZodOptional<z.ZodBoolean>;
    workerThreads: z.ZodOptional<z.ZodBoolean>;
    webVitalsAttribution: z.ZodOptional<z.ZodArray<z.ZodUnion<[z.ZodLiteral<"CLS">, z.ZodLiteral<"FCP">, z.ZodLiteral<"FID">, z.ZodLiteral<"INP">, z.ZodLiteral<"LCP">, z.ZodLiteral<"TTFB">]>, "many">>;
    mdxRs: z.ZodOptional<z.ZodUnion<[z.ZodBoolean, z.ZodObject<{
        development: z.ZodOptional<z.ZodBoolean>;
        jsxRuntime: z.ZodOptional<z.ZodString>;
        jsxImportSource: z.ZodOptional<z.ZodString>;
        providerImportSource: z.ZodOptional<z.ZodString>;
        mdxType: z.ZodOptional<z.ZodEnum<["gfm", "commonmark"]>>;
    }, "strip", z.ZodTypeAny, {
        development?: boolean | undefined;
        jsxImportSource?: string | undefined;
        jsxRuntime?: string | undefined;
        providerImportSource?: string | undefined;
        mdxType?: "gfm" | "commonmark" | undefined;
    }, {
        development?: boolean | undefined;
        jsxImportSource?: string | undefined;
        jsxRuntime?: string | undefined;
        providerImportSource?: string | undefined;
        mdxType?: "gfm" | "commonmark" | undefined;
    }>]>>;
    transitionIndicator: z.ZodOptional<z.ZodBoolean>;
    typedRoutes: z.ZodOptional<z.ZodBoolean>;
    webpackBuildWorker: z.ZodOptional<z.ZodBoolean>;
    webpackMemoryOptimizations: z.ZodOptional<z.ZodBoolean>;
    turbopackMemoryLimit: z.ZodOptional<z.ZodNumber>;
    turbopackMinify: z.ZodOptional<z.ZodBoolean>;
    turbopackFileSystemCacheForDev: z.ZodOptional<z.ZodBoolean>;
    turbopackFileSystemCacheForBuild: z.ZodOptional<z.ZodBoolean>;
    turbopackSourceMaps: z.ZodOptional<z.ZodBoolean>;
    turbopackInputSourceMaps: z.ZodOptional<z.ZodBoolean>;
    turbopackTreeShaking: z.ZodOptional<z.ZodBoolean>;
    turbopackRemoveUnusedImports: z.ZodOptional<z.ZodBoolean>;
    turbopackRemoveUnusedExports: z.ZodOptional<z.ZodBoolean>;
    turbopackScopeHoisting: z.ZodOptional<z.ZodBoolean>;
    turbopackClientSideNestedAsyncChunking: z.ZodOptional<z.ZodBoolean>;
    turbopackServerSideNestedAsyncChunking: z.ZodOptional<z.ZodBoolean>;
    turbopackImportTypeBytes: z.ZodOptional<z.ZodBoolean>;
    turbopackUseSystemTlsCerts: z.ZodOptional<z.ZodBoolean>;
    turbopackUseBuiltinBabel: z.ZodOptional<z.ZodBoolean>;
    turbopackUseBuiltinSass: z.ZodOptional<z.ZodBoolean>;
    turbopackModuleIds: z.ZodOptional<z.ZodEnum<["named", "deterministic"]>>;
    turbopackInferModuleSideEffects: z.ZodOptional<z.ZodBoolean>;
    optimizePackageImports: z.ZodOptional<z.ZodArray<z.ZodString, "many">>;
    optimizeServerReact: z.ZodOptional<z.ZodBoolean>;
    clientTraceMetadata: z.ZodOptional<z.ZodArray<z.ZodString, "many">>;
    serverMinification: z.ZodOptional<z.ZodBoolean>;
    serverSourceMaps: z.ZodOptional<z.ZodBoolean>;
    useWasmBinary: z.ZodOptional<z.ZodBoolean>;
    useLightningcss: z.ZodOptional<z.ZodBoolean>;
    testProxy: z.ZodOptional<z.ZodBoolean>;
    defaultTestRunner: z.ZodOptional<z.ZodEnum<["playwright"]>>;
    allowDevelopmentBuild: z.ZodOptional<z.ZodLiteral<true>>;
    reactDebugChannel: z.ZodOptional<z.ZodBoolean>;
    staticGenerationRetryCount: z.ZodOptional<z.ZodNumber>;
    staticGenerationMaxConcurrency: z.ZodOptional<z.ZodNumber>;
    staticGenerationMinPagesPerWorker: z.ZodOptional<z.ZodNumber>;
    typedEnv: z.ZodOptional<z.ZodBoolean>;
    serverComponentsHmrCache: z.ZodOptional<z.ZodBoolean>;
    authInterrupts: z.ZodOptional<z.ZodBoolean>;
    useCache: z.ZodOptional<z.ZodBoolean>;
    slowModuleDetection: z.ZodOptional<z.ZodObject<{
        buildTimeThresholdMs: z.ZodNumber;
    }, "strip", z.ZodTypeAny, {
        buildTimeThresholdMs: number;
    }, {
        buildTimeThresholdMs: number;
    }>>;
    globalNotFound: z.ZodOptional<z.ZodBoolean>;
    browserDebugInfoInTerminal: z.ZodOptional<z.ZodUnion<[z.ZodBoolean, z.ZodObject<{
        depthLimit: z.ZodOptional<z.ZodNumber>;
        edgeLimit: z.ZodOptional<z.ZodNumber>;
        showSourceLocation: z.ZodOptional<z.ZodBoolean>;
    }, "strip", z.ZodTypeAny, {
        depthLimit?: number | undefined;
        edgeLimit?: number | undefined;
        showSourceLocation?: boolean | undefined;
    }, {
        depthLimit?: number | undefined;
        edgeLimit?: number | undefined;
        showSourceLocation?: boolean | undefined;
    }>]>>;
    lockDistDir: z.ZodOptional<z.ZodBoolean>;
    hideLogsAfterAbort: z.ZodOptional<z.ZodBoolean>;
    runtimeServerDeploymentId: z.ZodOptional<z.ZodBoolean>;
};
export declare const configSchema: zod.ZodType<NextConfig>;
