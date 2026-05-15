import path from 'path';
import fs from 'fs/promises';
import { pathToFileURL } from 'url';
import * as Log from '../output/log';
import { isMiddlewareFilename } from '../utils';
import { RenderingMode } from '../rendering-mode';
import { interopDefault } from '../../lib/interop-default';
import { recursiveReadDir } from '../../lib/recursive-readdir';
import { isDynamicRoute } from '../../shared/lib/router/utils';
import { normalizeAppPath } from '../../shared/lib/router/utils/app-paths';
import { AdapterOutputType } from '../../shared/lib/constants';
import { normalizePagePath } from '../../shared/lib/page-path/normalize-page-path';
import { convertRedirects, convertRewrites, convertHeaders } from 'next/dist/compiled/@vercel/routing-utils';
import { HTML_CONTENT_TYPE_HEADER, JSON_CONTENT_TYPE_HEADER, NEXT_RESUME_HEADER } from '../../lib/constants';
import { normalizeLocalePath } from '../../shared/lib/i18n/normalize-locale-path';
import { addPathPrefix } from '../../shared/lib/router/utils/add-path-prefix';
import { getRedirectStatus, modifyRouteRegex } from '../../lib/redirect-status';
import { getNamedRouteRegex } from '../../shared/lib/router/utils/route-regex';
import { escapeStringRegexp } from '../../shared/lib/escape-regexp';
import { sortSortableRoutes } from '../../shared/lib/router/utils/sortable-routes';
import { nodeFileTrace } from 'next/dist/compiled/@vercel/nft';
import { defaultOverrides } from '../../server/require-hook';
import { makeIgnoreFn } from '../collect-build-traces';
function normalizePathnames(config, outputs) {
    // normalize pathname field with basePath
    if (config.basePath) {
        for (const output of [
            ...outputs.pages,
            ...outputs.pagesApi,
            ...outputs.appPages,
            ...outputs.appRoutes,
            ...outputs.prerenders,
            ...outputs.staticFiles
        ]){
            output.pathname = addPathPrefix(output.pathname, config.basePath).replace(/\/$/, '') || '/';
        }
    }
}
export async function handleBuildComplete({ dir, config, buildId, configOutDir, distDir, pageKeys, tracingRoot, adapterPath, appPageKeys, staticPages, nextVersion, hasStatic404, hasStatic500, routesManifest, serverPropsPages, hasNodeMiddleware, prerenderManifest, middlewareManifest, requiredServerFiles, hasInstrumentationHook, functionsConfigManifest }) {
    const adapterMod = interopDefault(await import(pathToFileURL(require.resolve(adapterPath)).href));
    if (typeof adapterMod.onBuildComplete === 'function') {
        const outputs = {
            pages: [],
            pagesApi: [],
            appPages: [],
            appRoutes: [],
            prerenders: [],
            staticFiles: []
        };
        if (config.output === 'export') {
            // collect export assets and provide as static files
            const exportFiles = await recursiveReadDir(configOutDir);
            for (const file of exportFiles){
                let pathname = (file.endsWith('.html') ? file.replace(/\.html$/, '') : file).replace(/\\/g, '/');
                pathname = pathname.startsWith('/') ? pathname : `/${pathname}`;
                outputs.staticFiles.push({
                    id: file,
                    pathname,
                    filePath: path.join(configOutDir, file),
                    type: AdapterOutputType.STATIC_FILE
                });
            }
        } else {
            const staticFiles = await recursiveReadDir(path.join(distDir, 'static'));
            for (const file of staticFiles){
                const pathname = path.posix.join('/_next/static', file);
                const filePath = path.join(distDir, 'static', file);
                outputs.staticFiles.push({
                    type: AdapterOutputType.STATIC_FILE,
                    id: path.join('static', file),
                    pathname,
                    filePath
                });
            }
            const sharedNodeAssets = {};
            const pagesSharedNodeAssets = {};
            const appPagesSharedNodeAssets = {};
            const sharedTraceIgnores = [
                '**/next/dist/compiled/next-server/**/*.dev.js',
                '**/next/dist/compiled/webpack/*',
                '**/node_modules/webpack5/**/*',
                '**/next/dist/server/lib/route-resolver*',
                'next/dist/compiled/semver/semver/**/*.js',
                '**/node_modules/react{,-dom,-dom-server-turbopack}/**/*.development.js',
                '**/*.d.ts',
                '**/*.map',
                '**/next/dist/pages/**/*',
                '**/node_modules/sharp/**/*',
                '**/@img/sharp-libvips*/**/*',
                '**/next/dist/compiled/edge-runtime/**/*',
                '**/next/dist/server/web/sandbox/**/*',
                '**/next/dist/server/post-process.js'
            ];
            const sharedIgnoreFn = makeIgnoreFn(tracingRoot, sharedTraceIgnores);
            for (const file of requiredServerFiles){
                // add to shared node assets
                const filePath = path.join(dir, file);
                const fileOutputPath = path.relative(tracingRoot, filePath);
                sharedNodeAssets[fileOutputPath] = filePath;
            }
            const moduleTypes = [
                'app-page',
                'pages'
            ];
            for (const type of moduleTypes){
                const currentDependencies = [];
                const modulePath = require.resolve(`next/dist/server/route-modules/${type}/module.compiled`);
                const contextDir = path.join(path.dirname(modulePath), 'vendored', 'contexts');
                for (const item of (await fs.readdir(contextDir))){
                    if (item.match(/\.(mjs|cjs|js)$/)) {
                        currentDependencies.push(path.join(contextDir, item));
                    }
                }
                const { fileList, esmFileList } = await nodeFileTrace(currentDependencies, {
                    base: tracingRoot,
                    ignore: sharedIgnoreFn
                });
                esmFileList.forEach((item)=>fileList.add(item));
                for (const rootRelativeFilePath of fileList){
                    if (type === 'pages') {
                        pagesSharedNodeAssets[rootRelativeFilePath] = path.join(tracingRoot, rootRelativeFilePath);
                    } else {
                        appPagesSharedNodeAssets[rootRelativeFilePath] = path.join(tracingRoot, rootRelativeFilePath);
                    }
                }
            }
            // These are modules that are necessary for bootstrapping node env
            const necessaryNodeDependencies = [
                require.resolve('next/dist/server/node-environment'),
                require.resolve('next/dist/server/require-hook'),
                require.resolve('next/dist/server/node-polyfill-crypto'),
                ...Object.values(defaultOverrides).filter((item)=>path.extname(item))
            ];
            const { fileList, esmFileList } = await nodeFileTrace(necessaryNodeDependencies, {
                base: tracingRoot,
                ignore: sharedIgnoreFn
            });
            esmFileList.forEach((item)=>fileList.add(item));
            for (const rootRelativeFilePath of fileList){
                sharedNodeAssets[rootRelativeFilePath] = path.join(tracingRoot, rootRelativeFilePath);
            }
            if (hasInstrumentationHook) {
                const assets = await handleTraceFiles(path.join(distDir, 'server', 'instrumentation.js.nft.json'), 'neutral');
                const fileOutputPath = path.relative(tracingRoot, path.join(distDir, 'server', 'instrumentation.js'));
                sharedNodeAssets[fileOutputPath] = path.join(distDir, 'server', 'instrumentation.js');
                Object.assign(sharedNodeAssets, assets);
            }
            async function handleTraceFiles(traceFilePath, type) {
                const assets = Object.assign({}, sharedNodeAssets, type === 'pages' ? pagesSharedNodeAssets : {}, type === 'app' ? appPagesSharedNodeAssets : {});
                const traceData = JSON.parse(await fs.readFile(traceFilePath, 'utf8'));
                const traceFileDir = path.dirname(traceFilePath);
                for (const relativeFile of traceData.files){
                    const tracedFilePath = path.join(traceFileDir, relativeFile);
                    const fileOutputPath = path.relative(tracingRoot, tracedFilePath);
                    assets[fileOutputPath] = tracedFilePath;
                }
                return assets;
            }
            async function handleEdgeFunction(page, isMiddleware = false) {
                let type = AdapterOutputType.PAGES;
                const isAppPrefix = page.name.startsWith('app/');
                const isAppPage = isAppPrefix && page.name.endsWith('/page');
                const isAppRoute = isAppPrefix && page.name.endsWith('/route');
                let currentOutputs = outputs.pages;
                if (isMiddleware) {
                    type = AdapterOutputType.MIDDLEWARE;
                } else if (isAppPage) {
                    currentOutputs = outputs.appPages;
                    type = AdapterOutputType.APP_PAGE;
                } else if (isAppRoute) {
                    currentOutputs = outputs.appRoutes;
                    type = AdapterOutputType.APP_ROUTE;
                } else if (page.page.startsWith('/api')) {
                    currentOutputs = outputs.pagesApi;
                    type = AdapterOutputType.PAGES_API;
                }
                const route = page.page.replace(/^(app|pages)\//, '');
                const output = {
                    type,
                    id: page.name,
                    runtime: 'edge',
                    sourcePage: route,
                    pathname: isAppPrefix ? normalizeAppPath(route) : route,
                    filePath: path.join(distDir, page.files.find((item)=>item.startsWith('server/app') || item.startsWith('server/pages')) || // TODO: turbopack build doesn't name the main entry chunk
                    // identifiably so we don't know which to mark here but
                    // technically edge needs all chunks to load always so
                    // should this field even be provided?
                    page.files[0] || ''),
                    assets: {},
                    wasmAssets: {},
                    config: {
                        env: page.env
                    }
                };
                function handleFile(file) {
                    const originalPath = path.join(distDir, file);
                    const fileOutputPath = path.join(path.relative(tracingRoot, distDir), file);
                    if (!output.assets) {
                        output.assets = {};
                    }
                    output.assets[fileOutputPath] = originalPath;
                }
                for (const file of page.files){
                    handleFile(file);
                }
                for (const item of [
                    ...page.assets || []
                ]){
                    handleFile(item.filePath);
                }
                for (const item of page.wasm || []){
                    if (!output.wasmAssets) {
                        output.wasmAssets = {};
                    }
                    output.wasmAssets[item.name] = path.join(distDir, item.filePath);
                }
                if (type === AdapterOutputType.MIDDLEWARE) {
                    ;
                    output.config.matchers = page.matchers.map((item)=>{
                        return {
                            source: item.originalSource,
                            sourceRegex: item.regexp,
                            has: item.has,
                            missing: [
                                ...item.missing || [],
                                // always skip middleware for on-demand revalidate
                                {
                                    type: 'header',
                                    key: 'x-prerender-revalidate',
                                    value: prerenderManifest.preview.previewModeId
                                }
                            ]
                        };
                    });
                    output.pathname = '/_middleware';
                    output.id = page.name;
                    outputs.middleware = output;
                } else {
                    currentOutputs.push(output);
                }
                // need to add matching .rsc output
                if (isAppPage) {
                    const rscPathname = normalizePagePath(output.pathname) + '.rsc';
                    outputs.appPages.push({
                        ...output,
                        pathname: rscPathname,
                        id: page.name + '.rsc'
                    });
                }
            }
            const edgeFunctionHandlers = [];
            for (const middleware of Object.values(middlewareManifest.middleware)){
                if (isMiddlewareFilename(middleware.name)) {
                    edgeFunctionHandlers.push(handleEdgeFunction(middleware, true));
                }
            }
            for (const page of Object.values(middlewareManifest.functions)){
                edgeFunctionHandlers.push(handleEdgeFunction(page));
            }
            const pagesDistDir = path.join(distDir, 'server', 'pages');
            const pageOutputMap = {};
            const rscFallbackPath = path.join(distDir, 'server', 'rsc-fallback.json');
            if (appPageKeys && appPageKeys.length > 0 && pageKeys.length > 0) {
                await fs.writeFile(rscFallbackPath, '{}');
            }
            for (const page of pageKeys){
                if (page === '/_app' || page === '/_document') {
                    continue;
                }
                if (middlewareManifest.functions.hasOwnProperty(page)) {
                    continue;
                }
                const route = normalizePagePath(page);
                const pageFile = path.join(pagesDistDir, `${route}.js`);
                // if it's an auto static optimized page it's just
                // a static file
                if (staticPages.has(page)) {
                    if (config.i18n) {
                        for (const locale of config.i18n.locales || []){
                            const localePage = page === '/' ? `/${locale}` : addPathPrefix(page, `/${locale}`);
                            const localeOutput = {
                                id: localePage,
                                pathname: localePage,
                                type: AdapterOutputType.STATIC_FILE,
                                filePath: path.join(pagesDistDir, `${normalizePagePath(localePage)}.html`)
                            };
                            outputs.staticFiles.push(localeOutput);
                            if (appPageKeys && appPageKeys.length > 0) {
                                outputs.staticFiles.push({
                                    id: `${localePage}.rsc`,
                                    pathname: `${localePage}.rsc`,
                                    type: AdapterOutputType.STATIC_FILE,
                                    filePath: rscFallbackPath
                                });
                            }
                        }
                    } else {
                        const staticOutput = {
                            id: page,
                            pathname: route,
                            type: AdapterOutputType.STATIC_FILE,
                            filePath: pageFile.replace(/\.js$/, '.html')
                        };
                        outputs.staticFiles.push(staticOutput);
                        if (appPageKeys && appPageKeys.length > 0) {
                            outputs.staticFiles.push({
                                id: `${page}.rsc`,
                                pathname: `${route}.rsc`,
                                type: AdapterOutputType.STATIC_FILE,
                                filePath: rscFallbackPath
                            });
                        }
                    }
                    continue;
                }
                const pageTraceFile = `${pageFile}.nft.json`;
                const assets = await handleTraceFiles(pageTraceFile, 'pages').catch((err)=>{
                    if (err.code !== 'ENOENT' || page !== '/404' && page !== '/500') {
                        Log.warn(`Failed to locate traced assets for ${pageFile}`, err);
                    }
                    return {};
                });
                const functionConfig = functionsConfigManifest.functions[route] || {};
                let sourcePage = route.replace(/^\//, '');
                sourcePage = sourcePage === 'api' ? 'api/index' : sourcePage;
                const output = {
                    id: route,
                    type: page.startsWith('/api') ? AdapterOutputType.PAGES_API : AdapterOutputType.PAGES,
                    filePath: pageTraceFile.replace(/\.nft\.json$/, ''),
                    pathname: route,
                    sourcePage,
                    assets,
                    runtime: 'nodejs',
                    config: {
                        maxDuration: functionConfig.maxDuration,
                        preferredRegion: functionConfig.regions
                    }
                };
                pageOutputMap[page] = output;
                if (output.type === AdapterOutputType.PAGES) {
                    var _config_i18n;
                    outputs.pages.push(output);
                    // if page is get server side props we need to create
                    // the _next/data output as well
                    if (serverPropsPages.has(page)) {
                        const dataPathname = path.posix.join('/_next/data', buildId, normalizePagePath(page) + '.json');
                        outputs.pages.push({
                            ...output,
                            pathname: dataPathname,
                            id: dataPathname
                        });
                    }
                    for (const locale of ((_config_i18n = config.i18n) == null ? void 0 : _config_i18n.locales) || []){
                        const localePage = page === '/' ? `/${locale}` : addPathPrefix(page, `/${locale}`);
                        outputs.pages.push({
                            ...output,
                            id: localePage,
                            pathname: localePage
                        });
                        if (serverPropsPages.has(page)) {
                            const dataPathname = path.posix.join('/_next/data', buildId, localePage + '.json');
                            outputs.pages.push({
                                ...output,
                                pathname: dataPathname,
                                id: dataPathname
                            });
                        }
                    }
                } else {
                    outputs.pagesApi.push(output);
                }
                if (appPageKeys && appPageKeys.length > 0) {
                    outputs.staticFiles.push({
                        id: `${output.id}.rsc`,
                        pathname: `${output.pathname}.rsc`,
                        type: AdapterOutputType.STATIC_FILE,
                        filePath: rscFallbackPath
                    });
                }
            }
            if (hasNodeMiddleware) {
                var _functionConfig_matchers;
                const middlewareFile = path.join(distDir, 'server', 'middleware.js');
                const middlewareTrace = `${middlewareFile}.nft.json`;
                const assets = await handleTraceFiles(middlewareTrace, 'neutral');
                const functionConfig = functionsConfigManifest.functions['/_middleware'] || {};
                outputs.middleware = {
                    pathname: '/_middleware',
                    id: '/_middleware',
                    sourcePage: 'middleware',
                    assets,
                    type: AdapterOutputType.MIDDLEWARE,
                    runtime: 'nodejs',
                    filePath: middlewareFile,
                    config: {
                        matchers: ((_functionConfig_matchers = functionConfig.matchers) == null ? void 0 : _functionConfig_matchers.map((item)=>{
                            return {
                                source: item.originalSource,
                                sourceRegex: item.regexp,
                                has: item.has,
                                missing: [
                                    ...item.missing || [],
                                    // always skip middleware for on-demand revalidate
                                    {
                                        type: 'header',
                                        key: 'x-prerender-revalidate',
                                        value: prerenderManifest.preview.previewModeId
                                    }
                                ]
                            };
                        })) || []
                    }
                };
            }
            const appOutputMap = {};
            const appDistDir = path.join(distDir, 'server', 'app');
            if (appPageKeys) {
                for (const page of appPageKeys){
                    if (middlewareManifest.functions.hasOwnProperty(page)) {
                        continue;
                    }
                    const normalizedPage = normalizeAppPath(page);
                    const pageFile = path.join(appDistDir, `${page}.js`);
                    const pageTraceFile = `${pageFile}.nft.json`;
                    const assets = await handleTraceFiles(pageTraceFile, 'app').catch((err)=>{
                        Log.warn(`Failed to copy traced files for ${pageFile}`, err);
                        return {};
                    });
                    // If this is a parallel route we just need to merge
                    // the assets as they share the same pathname
                    const existingOutput = appOutputMap[normalizedPage];
                    if (existingOutput) {
                        Object.assign(existingOutput.assets, assets);
                        existingOutput.assets[path.relative(tracingRoot, pageFile)] = pageFile;
                        continue;
                    }
                    const functionConfig = functionsConfigManifest.functions[normalizedPage] || {};
                    const output = {
                        pathname: normalizedPage,
                        id: normalizedPage,
                        sourcePage: page,
                        assets,
                        type: page.endsWith('/route') ? AdapterOutputType.APP_ROUTE : AdapterOutputType.APP_PAGE,
                        runtime: 'nodejs',
                        filePath: pageFile,
                        config: {
                            maxDuration: functionConfig.maxDuration,
                            preferredRegion: functionConfig.regions
                        }
                    };
                    appOutputMap[normalizedPage] = output;
                    if (output.type === AdapterOutputType.APP_PAGE) {
                        outputs.appPages.push({
                            ...output,
                            pathname: normalizePagePath(output.pathname) + '.rsc',
                            id: normalizePagePath(output.pathname) + '.rsc'
                        });
                        outputs.appPages.push(output);
                    } else {
                        outputs.appRoutes.push(output);
                    }
                }
            }
            const getParentOutput = (srcRoute, childRoute, allowMissing)=>{
                var _config_i18n;
                const normalizedSrcRoute = normalizeLocalePath(srcRoute, ((_config_i18n = config.i18n) == null ? void 0 : _config_i18n.locales) || []).pathname;
                const parentOutput = pageOutputMap[normalizedSrcRoute] || appOutputMap[normalizedSrcRoute];
                if (!parentOutput && !allowMissing) {
                    console.error({
                        appOutputs: Object.keys(appOutputMap),
                        pageOutputs: Object.keys(pageOutputMap)
                    });
                    throw Object.defineProperty(new Error(`Invariant: failed to find source route ${srcRoute} for prerender ${childRoute}`), "__NEXT_ERROR_CODE", {
                        value: "E777",
                        enumerable: false,
                        configurable: true
                    });
                }
                return parentOutput;
            };
            const { prefetchSegmentDirSuffix, prefetchSegmentSuffix, varyHeader, didPostponeHeader, contentTypeHeader: rscContentTypeHeader } = routesManifest.rsc;
            const handleAppMeta = async (route, initialOutput, meta)=>{
                if (meta.postponed && initialOutput.fallback) {
                    initialOutput.fallback.postponedState = meta.postponed;
                }
                if (meta == null ? void 0 : meta.segmentPaths) {
                    const normalizedRoute = normalizePagePath(route);
                    const segmentsDir = path.join(appDistDir, `${normalizedRoute}${prefetchSegmentDirSuffix}`);
                    for (const segmentPath of meta.segmentPaths){
                        var _initialOutput_fallback, _initialOutput_fallback1, _initialOutput_fallback2;
                        const outputSegmentPath = path.join(normalizedRoute + prefetchSegmentDirSuffix, segmentPath) + prefetchSegmentSuffix;
                        const fallbackPathname = path.join(segmentsDir, segmentPath + prefetchSegmentSuffix);
                        outputs.prerenders.push({
                            id: outputSegmentPath,
                            pathname: outputSegmentPath,
                            type: AdapterOutputType.PRERENDER,
                            parentOutputId: initialOutput.parentOutputId,
                            groupId: initialOutput.groupId,
                            config: {
                                ...initialOutput.config
                            },
                            fallback: {
                                filePath: fallbackPathname,
                                initialExpiration: (_initialOutput_fallback = initialOutput.fallback) == null ? void 0 : _initialOutput_fallback.initialExpiration,
                                initialRevalidate: (_initialOutput_fallback1 = initialOutput.fallback) == null ? void 0 : _initialOutput_fallback1.initialRevalidate,
                                initialHeaders: {
                                    ...(_initialOutput_fallback2 = initialOutput.fallback) == null ? void 0 : _initialOutput_fallback2.initialHeaders,
                                    vary: varyHeader,
                                    'content-type': rscContentTypeHeader,
                                    [didPostponeHeader]: '2'
                                }
                            }
                        });
                    }
                }
            };
            let prerenderGroupId = 1;
            const getAppRouteMeta = async (route, isAppPage)=>{
                const basename = route.endsWith('/') ? `${route}index` : route;
                const meta = isAppPage ? JSON.parse(await fs.readFile(path.join(appDistDir, `${basename}.meta`), 'utf8').catch(()=>'{}')) : {};
                if (meta.headers) {
                    // normalize these for consistency
                    for (const key of Object.keys(meta.headers)){
                        const keyLower = key.toLowerCase();
                        if (keyLower !== key) {
                            const value = meta.headers[key];
                            delete meta.headers[key];
                            meta.headers[keyLower] = value;
                        }
                    }
                }
                return meta;
            };
            const filePathCache = new Map();
            const cachedFilePathCheck = async (filePath)=>{
                if (filePathCache.has(filePath)) {
                    return filePathCache.get(filePath);
                }
                const newCheck = fs.access(filePath).then(()=>true).catch(()=>false);
                filePathCache.set(filePath, newCheck);
                return newCheck;
            };
            for(const route in prerenderManifest.routes){
                var _routesManifest_dynamicRoutes_find;
                const { initialExpireSeconds: initialExpiration, initialRevalidateSeconds: initialRevalidate, initialHeaders, initialStatus, prefetchDataRoute, dataRoute, renderingMode, allowHeader, experimentalBypassFor } = prerenderManifest.routes[route];
                const srcRoute = prerenderManifest.routes[route].srcRoute || route;
                const srcRouteInfo = prerenderManifest.dynamicRoutes[srcRoute];
                const isAppPage = Boolean(appOutputMap[srcRoute]) || srcRoute === '/_not-found';
                const isNotFoundTrue = prerenderManifest.notFoundRoutes.includes(route);
                let allowQuery;
                const routeKeys = (_routesManifest_dynamicRoutes_find = routesManifest.dynamicRoutes.find((item)=>item.page === srcRoute)) == null ? void 0 : _routesManifest_dynamicRoutes_find.routeKeys;
                if (!isDynamicRoute(srcRoute)) {
                    // for non-dynamic routes we use an empty array since
                    // no query values bust the cache for non-dynamic prerenders
                    // prerendered paths also do not pass allowQuery as they match
                    // during handle: 'filesystem' so should not cache differently
                    // by query values
                    allowQuery = [];
                } else if (routeKeys) {
                    // if we have routeKeys in the routes-manifest we use those
                    // for allowQuery for dynamic routes
                    allowQuery = Object.values(routeKeys);
                }
                let filePath = path.join(isAppPage ? appDistDir : pagesDistDir, `${normalizePagePath(route)}.${isAppPage && !dataRoute ? 'body' : 'html'}`);
                // we use the static 404 for notFound: true if available
                // if not we do a blocking invoke on first request
                if (isNotFoundTrue && hasStatic404) {
                    var _config_i18n1;
                    const locale = config.i18n && normalizeLocalePath(route, (_config_i18n1 = config.i18n) == null ? void 0 : _config_i18n1.locales).detectedLocale;
                    for (const currentFilePath of [
                        path.join(pagesDistDir, locale || '', '404.html'),
                        path.join(pagesDistDir, '404.html')
                    ]){
                        if (await cachedFilePathCheck(currentFilePath)) {
                            filePath = currentFilePath;
                            break;
                        }
                    }
                }
                const meta = await getAppRouteMeta(route, isAppPage);
                const initialOutput = {
                    id: route,
                    type: AdapterOutputType.PRERENDER,
                    pathname: route,
                    parentOutputId: srcRoute === '/_not-found' ? srcRoute : getParentOutput(srcRoute, route).id,
                    groupId: prerenderGroupId,
                    pprChain: isAppPage && config.experimental.ppr ? {
                        headers: {
                            [NEXT_RESUME_HEADER]: '1'
                        }
                    } : undefined,
                    parentFallbackMode: srcRouteInfo == null ? void 0 : srcRouteInfo.fallback,
                    fallback: !isNotFoundTrue || isNotFoundTrue && hasStatic404 ? {
                        filePath,
                        initialStatus: initialStatus ?? isNotFoundTrue ? 404 : undefined,
                        initialHeaders: {
                            ...initialHeaders,
                            vary: varyHeader,
                            'content-type': HTML_CONTENT_TYPE_HEADER,
                            ...meta.headers
                        },
                        initialExpiration,
                        initialRevalidate: typeof initialRevalidate === 'undefined' ? 1 : initialRevalidate
                    } : undefined,
                    config: {
                        allowQuery,
                        allowHeader,
                        renderingMode,
                        bypassFor: experimentalBypassFor,
                        bypassToken: prerenderManifest.preview.previewModeId
                    }
                };
                outputs.prerenders.push(initialOutput);
                if (dataRoute) {
                    var _initialOutput_fallback;
                    let dataFilePath = path.join(pagesDistDir, `${normalizePagePath(route)}.json`);
                    if (isAppPage) {
                        // When experimental PPR is enabled, we expect that the data
                        // that should be served as a part of the prerender should
                        // be from the prefetch data route. If this isn't enabled
                        // for ppr, the only way to get the data is from the data
                        // route.
                        dataFilePath = path.join(appDistDir, prefetchDataRoute && renderingMode === RenderingMode.PARTIALLY_STATIC ? prefetchDataRoute : dataRoute);
                    }
                    outputs.prerenders.push({
                        ...initialOutput,
                        id: dataRoute,
                        pathname: dataRoute,
                        fallback: isNotFoundTrue ? undefined : {
                            ...initialOutput.fallback,
                            initialHeaders: {
                                ...(_initialOutput_fallback = initialOutput.fallback) == null ? void 0 : _initialOutput_fallback.initialHeaders,
                                'content-type': isAppPage ? rscContentTypeHeader : JSON_CONTENT_TYPE_HEADER
                            },
                            filePath: dataFilePath
                        }
                    });
                }
                if (isAppPage) {
                    await handleAppMeta(route, initialOutput, meta);
                }
                prerenderGroupId += 1;
            }
            for(const dynamicRoute in prerenderManifest.dynamicRoutes){
                var _routesManifest_dynamicRoutes_find1;
                const { fallback, fallbackExpire, fallbackRevalidate, fallbackHeaders, fallbackStatus, allowHeader, dataRoute, renderingMode, experimentalBypassFor } = prerenderManifest.dynamicRoutes[dynamicRoute];
                const isAppPage = Boolean(appOutputMap[dynamicRoute]);
                const allowQuery = Object.values(((_routesManifest_dynamicRoutes_find1 = routesManifest.dynamicRoutes.find((item)=>item.page === dynamicRoute)) == null ? void 0 : _routesManifest_dynamicRoutes_find1.routeKeys) || {});
                const meta = await getAppRouteMeta(dynamicRoute, isAppPage);
                const initialOutput = {
                    id: dynamicRoute,
                    type: AdapterOutputType.PRERENDER,
                    pathname: dynamicRoute,
                    parentOutputId: getParentOutput(dynamicRoute, dynamicRoute).id,
                    groupId: prerenderGroupId,
                    config: {
                        allowQuery,
                        allowHeader,
                        renderingMode,
                        bypassFor: experimentalBypassFor,
                        bypassToken: prerenderManifest.preview.previewModeId
                    },
                    fallback: typeof fallback === 'string' ? {
                        filePath: path.join(isAppPage ? appDistDir : pagesDistDir, // app router dynamic route fallbacks don't have the
                        // extension so ensure it's added here
                        fallback.endsWith('.html') ? fallback : `${fallback}.html`),
                        initialStatus: fallbackStatus,
                        initialHeaders: {
                            ...fallbackHeaders,
                            'content-type': HTML_CONTENT_TYPE_HEADER
                        },
                        initialExpiration: fallbackExpire,
                        initialRevalidate: fallbackRevalidate || 1
                    } : undefined
                };
                if (!config.i18n || isAppPage) {
                    outputs.prerenders.push(initialOutput);
                    if (isAppPage) {
                        await handleAppMeta(dynamicRoute, initialOutput, meta);
                    }
                    if (dataRoute) {
                        outputs.prerenders.push({
                            ...initialOutput,
                            id: dataRoute,
                            pathname: dataRoute,
                            fallback: undefined
                        });
                    }
                    prerenderGroupId += 1;
                } else {
                    for (const locale of config.i18n.locales){
                        const currentOutput = {
                            ...initialOutput,
                            pathname: path.posix.join(`/${locale}`, initialOutput.pathname),
                            id: path.posix.join(`/${locale}`, initialOutput.id),
                            fallback: typeof fallback === 'string' ? {
                                ...initialOutput.fallback,
                                filePath: path.join(pagesDistDir, locale, // app router dynamic route fallbacks don't have the
                                // extension so ensure it's added here
                                fallback.endsWith('.html') ? fallback : `${fallback}.html`)
                            } : undefined,
                            groupId: prerenderGroupId
                        };
                        outputs.prerenders.push(currentOutput);
                        if (dataRoute) {
                            const dataPathname = path.posix.join(`/_next/data`, buildId, locale, dynamicRoute + '.json');
                            outputs.prerenders.push({
                                ...initialOutput,
                                id: dataPathname,
                                pathname: dataPathname,
                                // data route doesn't have skeleton fallback
                                fallback: undefined,
                                groupId: prerenderGroupId
                            });
                        }
                        prerenderGroupId += 1;
                    }
                }
            }
            // ensure 404
            const staticErrorDocs = [
                ...hasStatic404 ? [
                    '/404'
                ] : [],
                ...hasStatic500 ? [
                    '/500'
                ] : []
            ];
            for (const errorDoc of staticErrorDocs){
                var _config_i18n2;
                const errorDocPath = path.posix.join('/', ((_config_i18n2 = config.i18n) == null ? void 0 : _config_i18n2.defaultLocale) || '', errorDoc);
                if (!prerenderManifest.routes[errorDocPath]) {
                    var _config_i18n_locales, _config_i18n3;
                    for (const currentDocPath of [
                        errorDocPath,
                        ...((_config_i18n3 = config.i18n) == null ? void 0 : (_config_i18n_locales = _config_i18n3.locales) == null ? void 0 : _config_i18n_locales.map((locale)=>path.posix.join('/', locale, errorDoc))) || []
                    ]){
                        const currentFilePath = path.join(pagesDistDir, `${currentDocPath}.html`);
                        if (await cachedFilePathCheck(currentFilePath)) {
                            outputs.staticFiles.push({
                                pathname: currentDocPath,
                                id: currentDocPath,
                                type: AdapterOutputType.STATIC_FILE,
                                filePath: currentFilePath
                            });
                        }
                    }
                }
            }
        }
        normalizePathnames(config, outputs);
        const dynamicRoutes = [];
        const dynamicDataRoutes = [];
        const dynamicSegmentRoutes = [];
        const getDestinationQuery = (routeKeys)=>{
            const items = Object.entries(routeKeys ?? {});
            if (items.length === 0) return '';
            return '?' + items.map(([key, value])=>`${value}=$${key}`).join('&');
        };
        const fallbackFalseHasCondition = [
            {
                type: 'cookie',
                key: '__prerender_bypass',
                value: prerenderManifest.preview.previewModeId
            },
            {
                type: 'cookie',
                key: '__next_preview_data'
            }
        ];
        for (const route of routesManifest.dynamicRoutes){
            var _prerenderManifest_dynamicRoutes_route_page;
            const shouldLocalize = config.i18n;
            const routeRegex = getNamedRouteRegex(route.page, {
                prefixRouteKeys: true
            });
            const isFallbackFalse = ((_prerenderManifest_dynamicRoutes_route_page = prerenderManifest.dynamicRoutes[route.page]) == null ? void 0 : _prerenderManifest_dynamicRoutes_route_page.fallback) === false;
            const { hasFallbackRootParams } = route;
            const sourceRegex = routeRegex.namedRegex.replace('^', `^${config.basePath && config.basePath !== '/' ? path.posix.join('/', config.basePath || '') : ''}[/]?${shouldLocalize ? '(?<nextLocale>[^/]{1,})?' : ''}`);
            const destination = path.posix.join('/', config.basePath, shouldLocalize ? '/$nextLocale' : '', route.page) + getDestinationQuery(route.routeKeys);
            if (appPageKeys && appPageKeys.length > 0 && config.cacheComponents) {
                // If we have fallback root params (implying we've already
                // emitted a rewrite for the /_tree request), or if the route
                // has PPR enabled and client param parsing is enabled, then
                // we don't need to include any other suffixes.
                const shouldSkipSuffixes = hasFallbackRootParams;
                dynamicRoutes.push({
                    source: route.page + '.rsc',
                    sourceRegex: sourceRegex.replace(new RegExp(escapeStringRegexp('(?:/)?$')), // Now than the upstream issues has been resolved, we can safely
                    // add the suffix back, this resolves a bug related to segment
                    // rewrites not capturing the correct suffix values when
                    // enabled.
                    shouldSkipSuffixes ? '(?<rscSuffix>\\.rsc|\\.segments/.+\\.segment\\.rsc)(?:/)?$' : '(?<rscSuffix>\\.rsc|\\.prefetch\\.rsc|\\.segments/.+\\.segment\\.rsc)(?:/)?$'),
                    destination: destination == null ? void 0 : destination.replace(/($|\?)/, '$rscSuffix$1'),
                    has: isFallbackFalse ? fallbackFalseHasCondition : undefined,
                    missing: undefined
                });
            }
            // needs basePath and locale handling if pages router
            dynamicRoutes.push({
                source: route.page,
                sourceRegex,
                destination,
                has: isFallbackFalse ? fallbackFalseHasCondition : undefined,
                missing: undefined
            });
            for (const segmentRoute of route.prefetchSegmentDataRoutes || []){
                dynamicSegmentRoutes.push({
                    source: route.page,
                    sourceRegex: segmentRoute.source.replace('^', `^${config.basePath && config.basePath !== '/' ? path.posix.join('/', config.basePath || '') : ''}[/]?`),
                    destination: path.posix.join('/', config.basePath, segmentRoute.destination + getDestinationQuery(segmentRoute.routeKeys)),
                    has: undefined,
                    missing: undefined
                });
            }
        }
        const needsMiddlewareResolveRoutes = outputs.middleware && outputs.pages.length > 0;
        const dataRoutePages = new Set([
            ...routesManifest.dataRoutes.map((item)=>item.page)
        ]);
        const sortedDataPages = sortSortableRoutes([
            ...needsMiddlewareResolveRoutes ? [
                ...staticPages
            ].map((page)=>({
                    sourcePage: page,
                    page
                })) : [],
            ...routesManifest.dataRoutes.map((item)=>({
                    sourcePage: item.page,
                    page: item.page
                }))
        ]);
        for (const { page } of sortedDataPages){
            if (needsMiddlewareResolveRoutes || isDynamicRoute(page)) {
                var _prerenderManifest_dynamicRoutes_page;
                const shouldLocalize = config.i18n;
                const isFallbackFalse = ((_prerenderManifest_dynamicRoutes_page = prerenderManifest.dynamicRoutes[page]) == null ? void 0 : _prerenderManifest_dynamicRoutes_page.fallback) === false;
                const routeRegex = getNamedRouteRegex(page + '.json', {
                    prefixRouteKeys: true,
                    includeSuffix: true
                });
                const isDataRoute = dataRoutePages.has(page);
                const destination = path.posix.join('/', config.basePath, ...isDataRoute ? [
                    `_next/data`,
                    buildId
                ] : '', ...page === '/' ? [
                    shouldLocalize ? '$nextLocale.json' : 'index.json'
                ] : [
                    shouldLocalize ? '$nextLocale' : '',
                    page + (isDataRoute ? '.json' : '') + getDestinationQuery(routeRegex.routeKeys || {})
                ]);
                dynamicDataRoutes.push({
                    source: page,
                    sourceRegex: shouldLocalize && page === '/' ? '^' + path.posix.join('/', config.basePath, '_next/data', escapeStringRegexp(buildId), '(?<nextLocale>[^/]{1,}).json') : routeRegex.namedRegex.replace('^', `^${path.posix.join('/', config.basePath, `_next/data`, escapeStringRegexp(buildId))}[/]?${shouldLocalize ? '(?<nextLocale>[^/]{1,})?' : ''}`),
                    destination,
                    has: isFallbackFalse ? fallbackFalseHasCondition : undefined,
                    missing: undefined
                });
            }
        }
        const buildRewriteItem = (route)=>{
            const converted = convertRewrites([
                route
            ], [
                'nextInternalLocale'
            ])[0];
            const regex = converted.src || route.regex;
            return {
                source: route.source,
                sourceRegex: route.internal ? regex : modifyRouteRegex(regex),
                destination: converted.dest || route.destination,
                has: route.has,
                missing: route.missing
            };
        };
        try {
            Log.info(`Running onBuildComplete from ${adapterMod.name}`);
            await adapterMod.onBuildComplete({
                routes: {
                    dynamicRoutes: [
                        ...dynamicDataRoutes,
                        ...dynamicSegmentRoutes,
                        ...dynamicRoutes
                    ],
                    rewrites: {
                        beforeFiles: routesManifest.rewrites.beforeFiles.map(buildRewriteItem),
                        afterFiles: routesManifest.rewrites.afterFiles.map(buildRewriteItem),
                        fallback: routesManifest.rewrites.fallback.map(buildRewriteItem)
                    },
                    redirects: routesManifest.redirects.map((route)=>{
                        var _converted_headers;
                        const converted = convertRedirects([
                            route
                        ], 307)[0];
                        let dest = 'headers' in converted && ((_converted_headers = converted.headers) == null ? void 0 : _converted_headers.Location);
                        const regex = converted.src || route.regex;
                        return {
                            source: route.source,
                            sourceRegex: route.internal ? regex : modifyRouteRegex(regex),
                            destination: dest || route.destination,
                            statusCode: converted.status || getRedirectStatus(route),
                            has: route.has,
                            missing: route.missing,
                            priority: route.internal || undefined
                        };
                    }),
                    headers: routesManifest.headers.map((route)=>{
                        const converted = convertHeaders([
                            route
                        ])[0];
                        const regex = converted.src || route.regex;
                        return {
                            source: route.source,
                            sourceRegex: route.internal ? regex : modifyRouteRegex(regex),
                            headers: 'headers' in converted ? converted.headers || {} : {},
                            has: route.has,
                            missing: route.missing,
                            priority: route.internal || undefined
                        };
                    })
                },
                outputs,
                config,
                distDir,
                buildId,
                nextVersion,
                projectDir: dir,
                repoRoot: tracingRoot
            });
        } catch (err) {
            Log.error(`Failed to run onBuildComplete from ${adapterMod.name}`);
            throw err;
        }
    }
}

//# sourceMappingURL=build-complete.js.map