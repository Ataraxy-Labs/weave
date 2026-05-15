"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
0 && (module.exports = {
    DEFAULT_MAX_POSTPONED_STATE_SIZE: null,
    defaultConfig: null,
    getNextConfigRuntime: null,
    normalizeConfig: null,
    parseMaxPostponedStateSize: null
});
function _export(target, all) {
    for(var name in all)Object.defineProperty(target, name, {
        enumerable: true,
        get: all[name]
    });
}
_export(exports, {
    DEFAULT_MAX_POSTPONED_STATE_SIZE: function() {
        return _sizelimit.DEFAULT_MAX_POSTPONED_STATE_SIZE;
    },
    defaultConfig: function() {
        return defaultConfig;
    },
    getNextConfigRuntime: function() {
        return getNextConfigRuntime;
    },
    normalizeConfig: function() {
        return normalizeConfig;
    },
    parseMaxPostponedStateSize: function() {
        return _sizelimit.parseMaxPostponedStateSize;
    }
});
const _os = /*#__PURE__*/ _interop_require_default(require("os"));
const _imageconfig = require("../shared/lib/image-config");
const _constants = require("../lib/constants");
const _canaryonlyconfigerror = require("../shared/lib/errors/canary-only-config-error");
const _sizelimit = require("../shared/lib/size-limit");
function _interop_require_default(obj) {
    return obj && obj.__esModule ? obj : {
        default: obj
    };
}
const defaultConfig = Object.freeze({
    env: {},
    webpack: null,
    typescript: {
        ignoreBuildErrors: false,
        tsconfigPath: undefined
    },
    typedRoutes: false,
    distDir: '.next',
    cleanDistDir: true,
    assetPrefix: '',
    cacheHandler: process.env.NEXT_CACHE_HANDLER_PATH,
    // default to 50MB limit
    cacheMaxMemorySize: 50 * 1024 * 1024,
    configOrigin: 'default',
    useFileSystemPublicRoutes: true,
    generateBuildId: ()=>null,
    generateEtags: true,
    pageExtensions: [
        'tsx',
        'ts',
        'jsx',
        'js'
    ],
    poweredByHeader: true,
    compress: true,
    images: _imageconfig.imageConfigDefault,
    devIndicators: {
        position: 'bottom-left'
    },
    onDemandEntries: {
        maxInactiveAge: 60 * 1000,
        pagesBufferLength: 5
    },
    basePath: '',
    sassOptions: {},
    trailingSlash: false,
    i18n: null,
    productionBrowserSourceMaps: false,
    excludeDefaultMomentLocales: true,
    reactProductionProfiling: false,
    reactStrictMode: null,
    reactMaxHeadersLength: 6000,
    httpAgentOptions: {
        keepAlive: true
    },
    logging: {},
    compiler: {},
    expireTime: process.env.NEXT_PRIVATE_CDN_CONSUMED_SWR_CACHE_CONTROL ? undefined : 31536000,
    staticPageGenerationTimeout: 60,
    output: !!process.env.NEXT_PRIVATE_STANDALONE ? 'standalone' : undefined,
    modularizeImports: undefined,
    outputFileTracingRoot: process.env.NEXT_PRIVATE_OUTPUT_TRACE_ROOT || '',
    allowedDevOrigins: undefined,
    // Will default to cacheComponents value.
    enablePrerenderSourceMaps: undefined,
    cacheComponents: false,
    cacheLife: {
        default: {
            stale: undefined,
            revalidate: 60 * 15,
            expire: _constants.INFINITE_CACHE
        },
        seconds: {
            stale: 30,
            revalidate: 1,
            expire: 60
        },
        minutes: {
            stale: 60 * 5,
            revalidate: 60,
            expire: 60 * 60
        },
        hours: {
            stale: 60 * 5,
            revalidate: 60 * 60,
            expire: 60 * 60 * 24
        },
        days: {
            stale: 60 * 5,
            revalidate: 60 * 60 * 24,
            expire: 60 * 60 * 24 * 7
        },
        weeks: {
            stale: 60 * 5,
            revalidate: 60 * 60 * 24 * 7,
            expire: 60 * 60 * 24 * 30
        },
        max: {
            stale: 60 * 5,
            revalidate: 60 * 60 * 24 * 30,
            expire: 60 * 60 * 24 * 365
        }
    },
    cacheHandlers: {
        default: process.env.NEXT_DEFAULT_CACHE_HANDLER_PATH,
        remote: process.env.NEXT_REMOTE_CACHE_HANDLER_PATH,
        static: process.env.NEXT_STATIC_CACHE_HANDLER_PATH
    },
    experimental: {
        adapterPath: process.env.NEXT_ADAPTER_PATH || undefined,
        useSkewCookie: false,
        cssChunking: true,
        multiZoneDraftMode: false,
        appNavFailHandling: false,
        prerenderEarlyExit: true,
        serverMinification: true,
        linkNoTouchStart: false,
        caseSensitiveRoutes: false,
        clientParamParsingOrigins: undefined,
        dynamicOnHover: false,
        preloadEntriesOnStart: true,
        clientRouterFilter: true,
        clientRouterFilterRedirects: false,
        fetchCacheKeyPrefix: '',
        proxyPrefetch: 'flexible',
        optimisticClientCache: true,
        manualClientBasePath: false,
        cpus: Math.max(1, (Number(process.env.CIRCLE_NODE_TOTAL) || (_os.default.cpus() || {
            length: 1
        }).length) - 1),
        memoryBasedWorkersCount: false,
        imgOptConcurrency: null,
        imgOptTimeoutInSeconds: 7,
        imgOptMaxInputPixels: 268402689,
        imgOptSequentialRead: null,
        imgOptSkipMetadata: null,
        isrFlushToDisk: true,
        workerThreads: false,
        proxyTimeout: undefined,
        optimizeCss: false,
        nextScriptWorkers: false,
        scrollRestoration: false,
        externalDir: false,
        disableOptimizedLoading: false,
        gzipSize: true,
        craCompat: false,
        esmExternals: true,
        fullySpecified: false,
        swcTraceProfiling: false,
        forceSwcTransforms: false,
        swcPlugins: undefined,
        largePageDataBytes: 128 * 1000,
        disablePostcssPresetEnv: undefined,
        urlImports: undefined,
        typedEnv: false,
        clientTraceMetadata: undefined,
        parallelServerCompiles: false,
        parallelServerBuildTraces: false,
        ppr: false,
        authInterrupts: false,
        webpackBuildWorker: undefined,
        webpackMemoryOptimizations: false,
        optimizeServerReact: true,
        viewTransition: false,
        removeUncaughtErrorAndRejectionListeners: false,
        validateRSCRequestHeaders: !!(process.env.__NEXT_TEST_MODE || !(0, _canaryonlyconfigerror.isStableBuild)()),
        staleTimes: {
            dynamic: 0,
            static: 300
        },
        allowDevelopmentBuild: undefined,
        reactDebugChannel: false,
        staticGenerationRetryCount: undefined,
        serverComponentsHmrCache: true,
        staticGenerationMaxConcurrency: 8,
        staticGenerationMinPagesPerWorker: 25,
        transitionIndicator: false,
        inlineCss: false,
        useCache: undefined,
        slowModuleDetection: undefined,
        globalNotFound: false,
        browserDebugInfoInTerminal: false,
        lockDistDir: true,
        isolatedDevBuild: true,
        proxyClientMaxBodySize: 10485760,
        hideLogsAfterAbort: false,
        mcpServer: true,
        turbopackFileSystemCacheForDev: true,
        turbopackFileSystemCacheForBuild: false,
        turbopackInferModuleSideEffects: !(0, _canaryonlyconfigerror.isStableBuild)()
    },
    htmlLimitedBots: undefined,
    bundlePagesRouterDependencies: false
});
async function normalizeConfig(phase, config) {
    if (typeof config === 'function') {
        config = config(phase, {
            defaultConfig
        });
    }
    // Support `new Promise` and `async () =>` as return values of the config export
    return await config;
}
function getNextConfigRuntime(config) {
    // This config filter is a breaking change, so only do it if experimental.runtimeServerDeploymentId is enabled
    if (!config.experimental.runtimeServerDeploymentId) {
        return config;
    }
    let ex = config.experimental;
    let experimental = ex ? {
        ppr: ex.ppr,
        taint: ex.taint,
        serverActions: ex.serverActions,
        staleTimes: ex.staleTimes,
        dynamicOnHover: ex.dynamicOnHover,
        inlineCss: ex.inlineCss,
        authInterrupts: ex.authInterrupts,
        clientTraceMetadata: ex.clientTraceMetadata,
        clientParamParsingOrigins: ex.clientParamParsingOrigins,
        adapterPath: ex.adapterPath,
        allowedRevalidateHeaderKeys: ex.allowedRevalidateHeaderKeys,
        fetchCacheKeyPrefix: ex.fetchCacheKeyPrefix,
        isrFlushToDisk: ex.isrFlushToDisk,
        optimizeCss: ex.optimizeCss,
        nextScriptWorkers: ex.nextScriptWorkers,
        disableOptimizedLoading: ex.disableOptimizedLoading,
        largePageDataBytes: ex.largePageDataBytes,
        serverComponentsHmrCache: ex.serverComponentsHmrCache,
        caseSensitiveRoutes: ex.caseSensitiveRoutes,
        validateRSCRequestHeaders: ex.validateRSCRequestHeaders,
        sri: ex.sri,
        useSkewCookie: ex.useSkewCookie,
        preloadEntriesOnStart: ex.preloadEntriesOnStart,
        hideLogsAfterAbort: ex.hideLogsAfterAbort,
        removeUncaughtErrorAndRejectionListeners: ex.removeUncaughtErrorAndRejectionListeners,
        imgOptConcurrency: ex.imgOptConcurrency,
        imgOptMaxInputPixels: ex.imgOptMaxInputPixels,
        imgOptSequentialRead: ex.imgOptSequentialRead,
        imgOptSkipMetadata: ex.imgOptSkipMetadata,
        imgOptTimeoutInSeconds: ex.imgOptTimeoutInSeconds,
        proxyClientMaxBodySize: ex.proxyClientMaxBodySize,
        proxyTimeout: ex.proxyTimeout,
        testProxy: ex.testProxy,
        runtimeServerDeploymentId: ex.runtimeServerDeploymentId,
        maxPostponedStateSize: ex.maxPostponedStateSize,
        trustHostHeader: ex.trustHostHeader,
        isExperimentalCompile: ex.isExperimentalCompile
    } : {};
    let runtimeConfig = {
        deploymentId: config.experimental.runtimeServerDeploymentId ? '' : config.deploymentId,
        configFileName: undefined,
        env: undefined,
        distDir: config.distDir,
        cacheComponents: config.cacheComponents,
        htmlLimitedBots: config.htmlLimitedBots,
        assetPrefix: config.assetPrefix,
        output: config.output,
        crossOrigin: config.crossOrigin,
        trailingSlash: config.trailingSlash,
        images: config.images,
        reactMaxHeadersLength: config.reactMaxHeadersLength,
        cacheLife: config.cacheLife,
        basePath: config.basePath,
        expireTime: config.expireTime,
        generateEtags: config.generateEtags,
        poweredByHeader: config.poweredByHeader,
        cacheHandler: config.cacheHandler,
        cacheHandlers: config.cacheHandlers,
        cacheMaxMemorySize: config.cacheMaxMemorySize,
        compress: config.compress,
        i18n: config.i18n,
        httpAgentOptions: config.httpAgentOptions,
        skipProxyUrlNormalize: config.skipProxyUrlNormalize,
        pageExtensions: config.pageExtensions,
        useFileSystemPublicRoutes: config.useFileSystemPublicRoutes,
        experimental
    };
    if (config.experimental.isExperimentalCompile) {
        runtimeConfig.env = config.env;
    }
    return runtimeConfig;
}

//# sourceMappingURL=config-shared.js.map