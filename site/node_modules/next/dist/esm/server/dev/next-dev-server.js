import { addRequestMeta, getRequestMeta } from '../request-meta';
import * as React from 'react';
import fs from 'fs';
import { Worker } from 'next/dist/compiled/jest-worker';
import { join as pathJoin } from 'path';
import { PUBLIC_DIR_MIDDLEWARE_CONFLICT } from '../../lib/constants';
import { findPagesDir } from '../../lib/find-pages-dir';
import { PHASE_DEVELOPMENT_SERVER, PAGES_MANIFEST, APP_PATHS_MANIFEST, COMPILER_NAMES, PRERENDER_MANIFEST } from '../../shared/lib/constants';
import Server, { WrappedBuildError } from '../next-server';
import { normalizePagePath } from '../../shared/lib/page-path/normalize-page-path';
import { pathHasPrefix } from '../../shared/lib/router/utils/path-has-prefix';
import { removePathPrefix } from '../../shared/lib/router/utils/remove-path-prefix';
import { Telemetry } from '../../telemetry/storage';
import { setGlobal, trace } from '../../trace';
import { traceGlobals } from '../../trace/shared';
import { findPageFile } from '../lib/find-page-file';
import { getFormattedNodeOptionsWithoutInspect } from '../lib/utils';
import { withCoalescedInvoke } from '../../lib/coalesced-function';
import { loadDefaultErrorComponents } from '../load-default-error-components';
import { DecodeError, MiddlewareNotFoundError } from '../../shared/lib/utils';
import * as Log from '../../build/output/log';
import isError, { getProperError } from '../../lib/is-error';
import { defaultConfig } from '../config-shared';
import { isMiddlewareFile } from '../../build/utils';
import { formatServerError } from '../../lib/format-server-error';
import { DevRouteMatcherManager } from '../route-matcher-managers/dev-route-matcher-manager';
import { DevPagesRouteMatcherProvider } from '../route-matcher-providers/dev/dev-pages-route-matcher-provider';
import { DevPagesAPIRouteMatcherProvider } from '../route-matcher-providers/dev/dev-pages-api-route-matcher-provider';
import { DevAppPageRouteMatcherProvider } from '../route-matcher-providers/dev/dev-app-page-route-matcher-provider';
import { DevAppRouteRouteMatcherProvider } from '../route-matcher-providers/dev/dev-app-route-route-matcher-provider';
import { NodeManifestLoader } from '../route-matcher-providers/helpers/manifest-loaders/node-manifest-loader';
import { BatchedFileReader } from '../route-matcher-providers/dev/helpers/file-reader/batched-file-reader';
import { DefaultFileReader } from '../route-matcher-providers/dev/helpers/file-reader/default-file-reader';
import { LRUCache } from '../lib/lru-cache';
import { getMiddlewareRouteMatcher } from '../../shared/lib/router/utils/middleware-route-matcher';
import { DetachedPromise } from '../../lib/detached-promise';
import { isPostpone } from '../lib/router-utils/is-postpone';
import { generateInterceptionRoutesRewrites } from '../../lib/generate-interception-routes-rewrites';
import { buildCustomRoute } from '../../lib/build-custom-route';
import { decorateServerError } from '../../shared/lib/error-source';
import { logRequests } from './log-requests';
import { FallbackMode, fallbackModeToFallbackField } from '../../lib/fallback';
import { ensureInstrumentationRegistered, getInstrumentationModule } from '../lib/router-utils/instrumentation-globals.external';
import { getRouteRegex } from '../../shared/lib/router/utils/route-regex';
import { HMR_MESSAGE_SENT_TO_BROWSER } from './hot-reloader-types';
// Load ReactDevOverlay only when needed
let PagesDevOverlayBridgeImpl;
const ReactDevOverlay = (props)=>{
    if (PagesDevOverlayBridgeImpl === undefined) {
        PagesDevOverlayBridgeImpl = require('../../next-devtools/userspace/pages/pages-dev-overlay-setup').PagesDevOverlayBridge;
    }
    return React.createElement(PagesDevOverlayBridgeImpl, props);
};
export default class DevServer extends Server {
    getStaticPathsWorker() {
        const worker = new Worker(require.resolve('./static-paths-worker'), {
            maxRetries: 1,
            // For dev server, it's not necessary to spin up too many workers as long as you are not doing a load test.
            // This helps reusing the memory a lot.
            numWorkers: 1,
            enableWorkerThreads: this.nextConfig.experimental.workerThreads,
            forkOptions: {
                env: {
                    ...process.env,
                    // discard --inspect/--inspect-brk flags from process.env.NODE_OPTIONS. Otherwise multiple Node.js debuggers
                    // would be started if user launch Next.js in debugging mode. The number of debuggers is linked to
                    // the number of workers Next.js tries to launch. The only worker users are interested in debugging
                    // is the main Next.js one
                    NODE_OPTIONS: getFormattedNodeOptionsWithoutInspect()
                }
            }
        });
        worker.getStdout().pipe(process.stdout);
        worker.getStderr().pipe(process.stderr);
        return worker;
    }
    constructor(options){
        try {
            // Increase the number of stack frames on the server
            Error.stackTraceLimit = 50;
        } catch  {}
        super({
            ...options,
            dev: true
        }), /**
   * The promise that resolves when the server is ready. When this is unset
   * the server is ready.
   */ this.ready = new DetachedPromise();
        this.nextConfig = options.conf;
        this.bundlerService = options.bundlerService;
        this.startServerSpan = options.startServerSpan ?? trace('start-next-dev-server');
        this.renderOpts.dev = true;
        this.renderOpts.ErrorDebug = ReactDevOverlay;
        this.staticPathsCache = new LRUCache(// 5MB
        5 * 1024 * 1024, function length(value) {
            var _JSON_stringify;
            return ((_JSON_stringify = JSON.stringify(value.staticPaths)) == null ? void 0 : _JSON_stringify.length) ?? 0;
        });
        const { pagesDir, appDir } = findPagesDir(this.dir);
        this.pagesDir = pagesDir;
        this.appDir = appDir;
        if (this.nextConfig.experimental.serverComponentsHmrCache) {
            // Ensure HMR cache has a minimum size equal to the default cacheMaxMemorySize,
            // but allow it to grow if the user has configured a larger value.
            const hmrCacheSize = Math.max(this.nextConfig.cacheMaxMemorySize, defaultConfig.cacheMaxMemorySize);
            this.serverComponentsHmrCache = new LRUCache(hmrCacheSize, function length(value) {
                return JSON.stringify(value).length;
            });
        }
    }
    getServerComponentsHmrCache() {
        return this.serverComponentsHmrCache;
    }
    getRouteMatchers() {
        const { pagesDir, appDir } = findPagesDir(this.dir);
        const ensurer = {
            ensure: async (match, pathname)=>{
                await this.ensurePage({
                    definition: match.definition,
                    page: match.definition.page,
                    clientOnly: false,
                    url: pathname
                });
            }
        };
        const matchers = new DevRouteMatcherManager(super.getRouteMatchers(), ensurer, this.dir);
        const extensions = this.nextConfig.pageExtensions;
        const extensionsExpression = new RegExp(`\\.(?:${extensions.join('|')})$`);
        // If the pages directory is available, then configure those matchers.
        if (pagesDir) {
            const fileReader = new BatchedFileReader(new DefaultFileReader({
                // Only allow files that have the correct extensions.
                pathnameFilter: (pathname)=>extensionsExpression.test(pathname)
            }));
            matchers.push(new DevPagesRouteMatcherProvider(pagesDir, extensions, fileReader, this.localeNormalizer));
            matchers.push(new DevPagesAPIRouteMatcherProvider(pagesDir, extensions, fileReader, this.localeNormalizer));
        }
        if (appDir) {
            // We create a new file reader for the app directory because we don't want
            // to include any folders or files starting with an underscore. This will
            // prevent the reader from wasting time reading files that we know we
            // don't care about.
            const fileReader = new BatchedFileReader(new DefaultFileReader({
                // Ignore any directory prefixed with an underscore.
                ignorePartFilter: (part)=>part.startsWith('_')
            }));
            // TODO: Improve passing of "is running with Turbopack"
            const isTurbopack = !!process.env.TURBOPACK;
            matchers.push(new DevAppPageRouteMatcherProvider(appDir, extensions, fileReader, isTurbopack));
            matchers.push(new DevAppRouteRouteMatcherProvider(appDir, extensions, fileReader, isTurbopack));
        }
        return matchers;
    }
    getBuildId() {
        return 'development';
    }
    async prepareImpl() {
        var _this_ready;
        setGlobal('distDir', this.distDir);
        setGlobal('phase', PHASE_DEVELOPMENT_SERVER);
        // Use existing telemetry instance from traceGlobals instead of creating a new one.
        // Creating a new instance would overwrite the existing one, causing any telemetry
        // events recorded to the original instance to be lost during cleanup/flush.
        const existingTelemetry = traceGlobals.get('telemetry');
        const telemetry = existingTelemetry || new Telemetry({
            distDir: this.distDir
        });
        await super.prepareImpl();
        await this.matchers.reload();
        (_this_ready = this.ready) == null ? void 0 : _this_ready.resolve();
        this.ready = undefined;
        // In dev, this needs to be called after prepare because the build entries won't be known in the constructor
        this.interceptionRoutePatterns = this.getinterceptionRoutePatterns();
        // This is required by the tracing subsystem.
        setGlobal('appDir', this.appDir);
        setGlobal('pagesDir', this.pagesDir);
        // Only set telemetry if it wasn't already set
        if (!existingTelemetry) {
            setGlobal('telemetry', telemetry);
        }
        process.on('unhandledRejection', (reason)=>{
            if (isPostpone(reason)) {
                // React postpones that are unhandled might end up logged here but they're
                // not really errors. They're just part of rendering.
                return;
            }
            this.logErrorWithOriginalStack(reason, 'unhandledRejection');
        });
        process.on('uncaughtException', (err)=>{
            this.logErrorWithOriginalStack(err, 'uncaughtException');
        });
    }
    async hasPage(pathname) {
        let normalizedPath;
        try {
            normalizedPath = normalizePagePath(pathname);
        } catch (err) {
            console.error(err);
            // if normalizing the page fails it means it isn't valid
            // so it doesn't exist so don't throw and return false
            // to ensure we return 404 instead of 500
            return false;
        }
        if (isMiddlewareFile(normalizedPath)) {
            return findPageFile(this.dir, normalizedPath, this.nextConfig.pageExtensions, false).then(Boolean);
        }
        let appFile = null;
        let pagesFile = null;
        if (this.appDir) {
            appFile = await findPageFile(this.appDir, normalizedPath + '/page', this.nextConfig.pageExtensions, true);
        }
        if (this.pagesDir) {
            pagesFile = await findPageFile(this.pagesDir, normalizedPath, this.nextConfig.pageExtensions, false);
        }
        if (appFile && pagesFile) {
            return false;
        }
        return Boolean(appFile || pagesFile);
    }
    async runMiddleware(params) {
        try {
            const result = await super.runMiddleware({
                ...params,
                onWarning: (warn)=>{
                    this.logErrorWithOriginalStack(warn, 'warning');
                }
            });
            if ('finished' in result) {
                return result;
            }
            result.waitUntil.catch((error)=>{
                this.logErrorWithOriginalStack(error, 'unhandledRejection');
            });
            return result;
        } catch (error) {
            if (error instanceof DecodeError) {
                throw error;
            }
            /**
       * We only log the error when it is not a MiddlewareNotFound error as
       * in that case we should be already displaying a compilation error
       * which is what makes the module not found.
       */ if (!(error instanceof MiddlewareNotFoundError)) {
                this.logErrorWithOriginalStack(error);
            }
            const err = getProperError(error);
            decorateServerError(err, COMPILER_NAMES.edgeServer);
            const { request, response, parsedUrl } = params;
            /**
       * When there is a failure for an internal Next.js request from
       * middleware we bypass the error without finishing the request
       * so we can serve the required chunks to render the error.
       */ if (request.url.includes('/_next/static') || request.url.includes('/__nextjs_attach-nodejs-inspector') || request.url.includes('/__nextjs_original-stack-frame') || request.url.includes('/__nextjs_source-map') || request.url.includes('/__nextjs_error_feedback')) {
                return {
                    finished: false
                };
            }
            response.statusCode = 500;
            await this.renderError(err, request, response, parsedUrl.pathname);
            return {
                finished: true
            };
        }
    }
    async runEdgeFunction(params) {
        try {
            return super.runEdgeFunction({
                ...params,
                onError: (err)=>this.logErrorWithOriginalStack(err, 'app-dir'),
                onWarning: (warn)=>{
                    this.logErrorWithOriginalStack(warn, 'warning');
                }
            });
        } catch (error) {
            if (error instanceof DecodeError) {
                throw error;
            }
            this.logErrorWithOriginalStack(error, 'warning');
            const err = getProperError(error);
            const { req, res, page } = params;
            res.statusCode = 500;
            await this.renderError(err, req, res, page);
            return null;
        }
    }
    getRequestHandler() {
        const handler = super.getRequestHandler();
        return (req, res, parsedUrl)=>{
            const request = this.normalizeReq(req);
            const response = this.normalizeRes(res);
            const loggingConfig = this.nextConfig.logging;
            if (loggingConfig !== false) {
                // The closure variable is not used here because the request handler may be invoked twice for one request when middleware is added in the application.
                // By setting the start time we can ensure that the middleware timing is correctly included.
                if (!getRequestMeta(req, 'devRequestTimingStart')) {
                    const requestStart = process.hrtime.bigint();
                    addRequestMeta(req, 'devRequestTimingStart', requestStart);
                }
                const isMiddlewareRequest = getRequestMeta(req, 'middlewareInvoke') ?? false;
                if (!isMiddlewareRequest) {
                    response.originalResponse.once('close', ()=>{
                        // NOTE: The route match is only attached to the request's meta data
                        // after the request handler is created, so we need to check it in the
                        // close handler and not before.
                        const routeMatch = getRequestMeta(req).match;
                        if (!routeMatch) {
                            return;
                        }
                        // The closure variable is not used here because the request handler may be invoked twice for one request when middleware is added in the application.
                        // By setting the start time we can ensure that the middleware timing is correctly included.
                        const requestStart = getRequestMeta(req, 'devRequestTimingStart');
                        if (!requestStart) {
                            return;
                        }
                        const requestEnd = process.hrtime.bigint();
                        logRequests(request, response, loggingConfig, requestStart, requestEnd, getRequestMeta(req, 'devRequestTimingMiddlewareStart'), getRequestMeta(req, 'devRequestTimingMiddlewareEnd'), getRequestMeta(req, 'devRequestTimingInternalsEnd'), getRequestMeta(req, 'devGenerateStaticParamsDuration'));
                    });
                }
            }
            return handler(request, response, parsedUrl);
        };
    }
    async handleRequest(req, res, parsedUrl) {
        const span = trace('handle-request', undefined, {
            url: req.url
        });
        const result = await span.traceAsyncFn(async ()=>{
            var _this_ready;
            await ((_this_ready = this.ready) == null ? void 0 : _this_ready.promise);
            addRequestMeta(req, 'PagesErrorDebug', this.renderOpts.ErrorDebug);
            return await super.handleRequest(req, res, parsedUrl);
        });
        const memoryUsage = process.memoryUsage();
        span.traceChild('memory-usage', {
            url: req.url,
            'memory.rss': String(memoryUsage.rss),
            'memory.heapUsed': String(memoryUsage.heapUsed),
            'memory.heapTotal': String(memoryUsage.heapTotal)
        }).stop();
        return result;
    }
    async run(req, res, parsedUrl) {
        var _this_ready;
        await ((_this_ready = this.ready) == null ? void 0 : _this_ready.promise);
        const { basePath } = this.nextConfig;
        let originalPathname = null;
        // TODO: see if we can remove this in the future
        if (basePath && pathHasPrefix(parsedUrl.pathname || '/', basePath)) {
            // strip basePath before handling dev bundles
            // If replace ends up replacing the full url it'll be `undefined`, meaning we have to default it to `/`
            originalPathname = parsedUrl.pathname;
            parsedUrl.pathname = removePathPrefix(parsedUrl.pathname || '/', basePath);
        }
        const { pathname } = parsedUrl;
        if (pathname.startsWith('/_next')) {
            if (fs.existsSync(pathJoin(this.publicDir, '_next'))) {
                throw Object.defineProperty(new Error(PUBLIC_DIR_MIDDLEWARE_CONFLICT), "__NEXT_ERROR_CODE", {
                    value: "E394",
                    enumerable: false,
                    configurable: true
                });
            }
        }
        if (originalPathname) {
            // restore the path before continuing so that custom-routes can accurately determine
            // if they should match against the basePath or not
            parsedUrl.pathname = originalPathname;
        }
        try {
            return await super.run(req, res, parsedUrl);
        } catch (error) {
            const err = getProperError(error);
            formatServerError(err);
            this.logErrorWithOriginalStack(err);
            if (!res.sent) {
                res.statusCode = 500;
                try {
                    return await this.renderError(err, req, res, pathname, {
                        __NEXT_PAGE: isError(err) && err.page || pathname || ''
                    });
                } catch (internalErr) {
                    console.error(internalErr);
                    res.body('Internal Server Error').send();
                }
            }
        }
    }
    logErrorWithOriginalStack(err, type) {
        this.bundlerService.logErrorWithOriginalStack(err, type);
    }
    getPagesManifest() {
        return NodeManifestLoader.require(pathJoin(this.serverDistDir, PAGES_MANIFEST)) ?? undefined;
    }
    getAppPathsManifest() {
        if (!this.enabledDirectories.app) return undefined;
        return NodeManifestLoader.require(pathJoin(this.serverDistDir, APP_PATHS_MANIFEST)) ?? undefined;
    }
    getinterceptionRoutePatterns() {
        const rewrites = generateInterceptionRoutesRewrites(Object.keys(this.appPathRoutes ?? {}), this.nextConfig.basePath).map((route)=>new RegExp(buildCustomRoute('rewrite', route).regex));
        if (this.nextConfig.output === 'export' && rewrites.length > 0) {
            Log.error('Intercepting routes are not supported with static export.\nRead more: https://nextjs.org/docs/app/building-your-application/deploying/static-exports#unsupported-features');
            process.exit(1);
        }
        return rewrites ?? [];
    }
    async getMiddleware() {
        var _this_middleware;
        // We need to populate the match
        // field as it isn't serializable
        if (((_this_middleware = this.middleware) == null ? void 0 : _this_middleware.match) === null) {
            this.middleware.match = getMiddlewareRouteMatcher(this.middleware.matchers || []);
        }
        return this.middleware;
    }
    getNextFontManifest() {
        return undefined;
    }
    async hasMiddleware() {
        return this.hasPage(this.actualMiddlewareFile);
    }
    async ensureMiddleware(url) {
        return this.ensurePage({
            page: this.actualMiddlewareFile,
            clientOnly: false,
            definition: undefined,
            url
        });
    }
    async loadInstrumentationModule() {
        let instrumentationModule;
        if (this.actualInstrumentationHookFile && await this.ensurePage({
            page: this.actualInstrumentationHookFile,
            clientOnly: false,
            definition: undefined
        }).then(()=>true).catch(()=>false)) {
            try {
                instrumentationModule = await getInstrumentationModule(this.dir, this.nextConfig.distDir);
            } catch (err) {
                err.message = `An error occurred while loading instrumentation hook: ${err.message}`;
                throw err;
            }
        }
        return instrumentationModule;
    }
    async runInstrumentationHookIfAvailable() {
        await ensureInstrumentationRegistered(this.dir, this.nextConfig.distDir);
    }
    async ensureEdgeFunction({ page, appPaths, url }) {
        return this.ensurePage({
            page,
            appPaths,
            clientOnly: false,
            definition: undefined,
            url
        });
    }
    generateRoutes(_dev) {
    // In development we expose all compiled files for react-error-overlay's line show feature
    // We use unshift so that we're sure the routes is defined before Next's default routes
    // routes.unshift({
    //   match: getPathMatch('/_next/development/:path*'),
    //   type: 'route',
    //   name: '_next/development catchall',
    //   fn: async (req, res, params) => {
    //     const p = pathJoin(this.distDir, ...(params.path || []))
    //     await this.serveStatic(req, res, p)
    //     return {
    //       finished: true,
    //     }
    //   },
    // })
    }
    async getStaticPaths({ pathname, urlPathname, requestHeaders, page, isAppPath }) {
        // we lazy load the staticPaths to prevent the user
        // from waiting on them for the page to load in dev mode
        const __getStaticPaths = async ()=>{
            const { configFileName, httpAgentOptions } = this.nextConfig;
            const { locales, defaultLocale } = this.nextConfig.i18n || {};
            const staticPathsWorker = this.getStaticPathsWorker();
            try {
                var _this_nextConfig_experimental_sri;
                const pathsResult = await staticPathsWorker.loadStaticPaths({
                    dir: this.dir,
                    distDir: this.distDir,
                    pathname,
                    config: {
                        pprConfig: this.nextConfig.experimental.ppr,
                        configFileName,
                        cacheComponents: Boolean(this.nextConfig.cacheComponents)
                    },
                    httpAgentOptions,
                    locales,
                    defaultLocale,
                    page,
                    isAppPath,
                    requestHeaders,
                    cacheHandler: this.nextConfig.cacheHandler,
                    cacheHandlers: this.nextConfig.cacheHandlers,
                    cacheLifeProfiles: this.nextConfig.cacheLife,
                    fetchCacheKeyPrefix: this.nextConfig.experimental.fetchCacheKeyPrefix,
                    isrFlushToDisk: this.nextConfig.experimental.isrFlushToDisk,
                    cacheMaxMemorySize: this.nextConfig.cacheMaxMemorySize,
                    nextConfigOutput: this.nextConfig.output,
                    buildId: this.buildId,
                    authInterrupts: Boolean(this.nextConfig.experimental.authInterrupts),
                    sriEnabled: Boolean((_this_nextConfig_experimental_sri = this.nextConfig.experimental.sri) == null ? void 0 : _this_nextConfig_experimental_sri.algorithm)
                });
                return pathsResult;
            } finally{
                // we don't re-use workers so destroy the used one
                staticPathsWorker.end();
            }
        };
        const result = this.staticPathsCache.get(pathname);
        const nextInvoke = withCoalescedInvoke(__getStaticPaths)(`staticPaths-${pathname}`, []).then(async (res)=>{
            var _res_value;
            const { prerenderedRoutes, fallbackMode: fallback } = res.value;
            if (isAppPath) {
                var // Ideally, we would want to compare the whole objects, but that is too expensive.
                _result_prerenderedRoutes;
                if (this.nextConfig.output === 'export') {
                    if (!prerenderedRoutes) {
                        throw Object.defineProperty(new Error(`Page "${page}" is missing exported function "generateStaticParams()", which is required with "output: export" config.`), "__NEXT_ERROR_CODE", {
                            value: "E353",
                            enumerable: false,
                            configurable: true
                        });
                    }
                    if (!prerenderedRoutes.some((item)=>item.pathname === urlPathname)) {
                        throw Object.defineProperty(new Error(`Page "${page}" is missing param "${pathname}" in "generateStaticParams()", which is required with "output: export" config.`), "__NEXT_ERROR_CODE", {
                            value: "E443",
                            enumerable: false,
                            configurable: true
                        });
                    }
                }
                // Since generateStaticParams run on the background, when accessing the
                // devFallbackParams during the render, it is still set to the previous
                // result from the cache. Therefore when the result has changed, re-render
                // the Server Component to sync the devFallbackParams with the new result.
                if (isAppPath && this.nextConfig.cacheComponents && // Ensure this is not the first invocation.
                result && ((_result_prerenderedRoutes = result.prerenderedRoutes) == null ? void 0 : _result_prerenderedRoutes.length) !== (prerenderedRoutes == null ? void 0 : prerenderedRoutes.length)) {
                    this.bundlerService.sendHmrMessage({
                        type: HMR_MESSAGE_SENT_TO_BROWSER.SERVER_COMPONENT_CHANGES,
                        hash: `generateStaticParams-${Date.now()}`
                    });
                }
            }
            if (!isAppPath && this.nextConfig.output === 'export') {
                if (fallback === FallbackMode.BLOCKING_STATIC_RENDER) {
                    throw Object.defineProperty(new Error('getStaticPaths with "fallback: blocking" cannot be used with "output: export". See more info here: https://nextjs.org/docs/advanced-features/static-html-export'), "__NEXT_ERROR_CODE", {
                        value: "E11",
                        enumerable: false,
                        configurable: true
                    });
                } else if (fallback === FallbackMode.PRERENDER) {
                    throw Object.defineProperty(new Error('getStaticPaths with "fallback: true" cannot be used with "output: export". See more info here: https://nextjs.org/docs/advanced-features/static-html-export'), "__NEXT_ERROR_CODE", {
                        value: "E210",
                        enumerable: false,
                        configurable: true
                    });
                }
            }
            const value = {
                staticPaths: prerenderedRoutes == null ? void 0 : prerenderedRoutes.map((route)=>route.pathname),
                prerenderedRoutes,
                fallbackMode: fallback
            };
            if (((_res_value = res.value) == null ? void 0 : _res_value.fallbackMode) !== undefined && // This matches the hasGenerateStaticParams logic we do during build.
            (!isAppPath || prerenderedRoutes && prerenderedRoutes.length > 0)) {
                // we write the static paths to partial manifest for
                // fallback handling inside of entry handler's
                const rawExistingManifest = await fs.promises.readFile(pathJoin(this.distDir, PRERENDER_MANIFEST), 'utf8');
                const existingManifest = JSON.parse(rawExistingManifest);
                for (const staticPath of value.staticPaths || []){
                    existingManifest.routes[staticPath] = {};
                }
                existingManifest.dynamicRoutes[pathname] = {
                    dataRoute: null,
                    dataRouteRegex: null,
                    fallback: fallbackModeToFallbackField(res.value.fallbackMode, page),
                    fallbackRevalidate: false,
                    fallbackExpire: undefined,
                    fallbackHeaders: undefined,
                    fallbackStatus: undefined,
                    fallbackRootParams: undefined,
                    fallbackRouteParams: undefined,
                    fallbackSourceRoute: pathname,
                    prefetchDataRoute: undefined,
                    prefetchDataRouteRegex: undefined,
                    routeRegex: getRouteRegex(pathname).re.source,
                    experimentalPPR: undefined,
                    renderingMode: undefined,
                    allowHeader: []
                };
                const updatedManifest = JSON.stringify(existingManifest);
                if (updatedManifest !== rawExistingManifest) {
                    await fs.promises.writeFile(pathJoin(this.distDir, PRERENDER_MANIFEST), updatedManifest);
                }
            }
            this.staticPathsCache.set(pathname, value);
            return value;
        }).catch((err)=>{
            this.staticPathsCache.remove(pathname);
            if (!result) throw err;
            Log.error(`Failed to generate static paths for ${pathname}:`);
            console.error(err);
        });
        if (result) {
            return result;
        }
        return nextInvoke;
    }
    async ensurePage(opts) {
        await this.bundlerService.ensurePage(opts);
    }
    async findPageComponents({ locale, page, query, params, isAppPath, appPaths = null, shouldEnsure, url }) {
        var _this_ready;
        await ((_this_ready = this.ready) == null ? void 0 : _this_ready.promise);
        const compilationErr = await this.getCompilationError(page);
        if (compilationErr) {
            // Wrap build errors so that they don't get logged again
            throw new WrappedBuildError(compilationErr);
        }
        if (shouldEnsure || this.serverOptions.customServer) {
            await this.ensurePage({
                page,
                appPaths,
                clientOnly: false,
                definition: undefined,
                url
            });
        }
        this.nextFontManifest = super.getNextFontManifest();
        return await super.findPageComponents({
            page,
            query,
            params,
            locale,
            isAppPath,
            shouldEnsure,
            url
        });
    }
    async getFallbackErrorComponents(url) {
        await this.bundlerService.getFallbackErrorComponents(url);
        return await loadDefaultErrorComponents(this.distDir);
    }
    async getCompilationError(page) {
        return await this.bundlerService.getCompilationError(page);
    }
    async instrumentationOnRequestError(...args) {
        await super.instrumentationOnRequestError(...args);
        const [err, , , silenceLog] = args;
        if (!silenceLog) {
            this.logErrorWithOriginalStack(err, 'app-dir');
        }
    }
}

//# sourceMappingURL=next-dev-server.js.map