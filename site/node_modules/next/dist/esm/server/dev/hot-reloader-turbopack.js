import { mkdir, writeFile } from 'fs/promises';
import * as inspector from 'inspector';
import { join, extname, relative } from 'path';
import { pathToFileURL } from 'url';
import ws from 'next/dist/compiled/ws';
import { store as consoleStore } from '../../build/output/store';
import { HMR_MESSAGE_SENT_TO_BROWSER } from './hot-reloader-types';
import { createDefineEnv, getBindingsSync } from '../../build/swc';
import * as Log from '../../build/output/log';
import { BLOCKED_PAGES } from '../../shared/lib/constants';
import { getOverlayMiddleware, getSourceMapMiddleware, getOriginalStackFrames } from './middleware-turbopack';
import { PageNotFoundError } from '../../shared/lib/utils';
import { debounce } from '../utils';
import { deleteCache } from './require-cache';
import { clearAllModuleContexts, clearModuleContext } from '../lib/render-server';
import { denormalizePagePath } from '../../shared/lib/page-path/denormalize-page-path';
import { trace } from '../../trace';
import { AssetMapper, handleEntrypoints, handlePagesErrorRoute, handleRouteType, hasEntrypointForKey, msToNs, processTopLevelIssues, printNonFatalIssue, normalizedPageToTurbopackStructureRoute } from './turbopack-utils';
import { propagateServerField } from '../lib/router-utils/setup-dev-bundler';
import { TurbopackManifestLoader } from '../../shared/lib/turbopack/manifest-loader';
import { findPagePathData } from './on-demand-entry-handler';
import { getEntryKey, splitEntryKey } from '../../shared/lib/turbopack/entry-key';
import { createBinaryHmrMessageData, FAST_REFRESH_RUNTIME_RELOAD } from './messages';
import { generateEncryptionKeyBase64 } from '../app-render/encryption-utils-server';
import { isAppPageRouteDefinition } from '../route-definitions/app-page-route-definition';
import { normalizeAppPath } from '../../shared/lib/router/utils/app-paths';
import { isMetadataRouteFile } from '../../lib/metadata/is-metadata-route';
import { setBundlerFindSourceMapImplementation } from '../patch-error-inspect';
import { getNextErrorFeedbackMiddleware } from '../../next-devtools/server/get-next-error-feedback-middleware';
import { formatIssue, isFileSystemCacheEnabledForDev, isWellKnownError, processIssues, renderStyledStringToErrorAnsi } from '../../shared/lib/turbopack/utils';
import { getDevOverlayFontMiddleware } from '../../next-devtools/server/font/get-dev-overlay-font-middleware';
import { devIndicatorServerState } from './dev-indicator-server-state';
import { getDisableDevIndicatorMiddleware } from '../../next-devtools/server/dev-indicator-middleware';
import { getRestartDevServerMiddleware } from '../../next-devtools/server/restart-dev-server-middleware';
import { backgroundLogCompilationEvents } from '../../shared/lib/turbopack/compilation-events';
import { getSupportedBrowsers, printBuildErrors } from '../../build/utils';
import { receiveBrowserLogsTurbopack, handleClientFileLogs } from './browser-logs/receive-logs';
import { normalizePath } from '../../lib/normalize-path';
import { devToolsConfigMiddleware, getDevToolsConfig } from '../../next-devtools/server/devtools-config-middleware';
import { getAttachNodejsDebuggerMiddleware } from '../../next-devtools/server/attach-nodejs-debugger-middleware';
import { connectReactDebugChannel, connectReactDebugChannelForHtmlRequest, deleteReactDebugChannelForHtmlRequest, setReactDebugChannelForHtmlRequest } from './debug-channel';
import { getVersionInfo, matchNextPageBundleRequest } from './hot-reloader-shared-utils';
import { getMcpMiddleware } from '../mcp/get-mcp-middleware';
import { handleErrorStateResponse } from '../mcp/tools/get-errors';
import { handlePageMetadataResponse } from '../mcp/tools/get-page-metadata';
import { setStackFrameResolver } from '../mcp/tools/utils/format-errors';
import { recordMcpTelemetry } from '../mcp/mcp-telemetry-tracker';
import { getFileLogger } from './browser-logs/file-logger';
import { sendSerializedErrorsToClient, sendSerializedErrorsToClientForHtmlRequest, setErrorsRscStreamForHtmlRequest } from './serialized-errors';
const wsServer = new ws.Server({
    noServer: true
});
const isTestMode = !!(process.env.NEXT_TEST_MODE || process.env.__NEXT_TEST_MODE || process.env.DEBUG);
const sessionId = Math.floor(Number.MAX_SAFE_INTEGER * Math.random());
/**
 * Replaces turbopack:///[project] with the specified project in the `source` field.
 */ function rewriteTurbopackSources(projectRoot, sourceMap) {
    if ('sections' in sourceMap) {
        for (const section of sourceMap.sections){
            rewriteTurbopackSources(projectRoot, section.map);
        }
    } else {
        for(let i = 0; i < sourceMap.sources.length; i++){
            sourceMap.sources[i] = pathToFileURL(join(projectRoot, sourceMap.sources[i].replace(/turbopack:\/\/\/\[project\]/, ''))).toString();
        }
    }
}
function getSourceMapFromTurbopack(project, projectRoot, sourceURL) {
    let sourceMapJson = null;
    try {
        sourceMapJson = project.getSourceMapSync(sourceURL);
    } catch (err) {}
    if (sourceMapJson === null) {
        return undefined;
    } else {
        const payload = JSON.parse(sourceMapJson);
        // The sourcemap from Turbopack is not yet written to disk so its `sources`
        // are not absolute paths yet. We need to rewrite them to be absolute paths.
        rewriteTurbopackSources(projectRoot, payload);
        return payload;
    }
}
export async function createHotReloaderTurbopack(opts, serverFields, distDir, resetFetch, lockfile) {
    var _opts_nextConfig_turbopack, _nextConfig_watchOptions, _opts_nextConfig_experimental;
    const dev = true;
    const buildId = 'development';
    const { nextConfig, dir: projectPath } = opts;
    const bindings = getBindingsSync();
    // For the debugging purpose, check if createNext or equivalent next instance setup in test cases
    // works correctly. Normally `run-test` hides output so only will be visible when `--debug` flag is used.
    if (isTestMode) {
        ;
        require('console').log('Creating turbopack project', {
            dir: projectPath,
            testMode: isTestMode
        });
    }
    const hasRewrites = opts.fsChecker.rewrites.afterFiles.length > 0 || opts.fsChecker.rewrites.beforeFiles.length > 0 || opts.fsChecker.rewrites.fallback.length > 0;
    const hotReloaderSpan = trace('hot-reloader', undefined, {
        version: "16.1.6"
    });
    // Ensure the hotReloaderSpan is flushed immediately as it's the parentSpan for all processing
    // of the current `next dev` invocation.
    hotReloaderSpan.stop();
    // Initialize log monitor for file logging
    // Enable logging by default in development mode
    const mcpServerEnabled = !!nextConfig.experimental.mcpServer;
    const fileLogger = getFileLogger();
    fileLogger.initialize(distDir, mcpServerEnabled);
    const encryptionKey = await generateEncryptionKeyBase64({
        isBuild: false,
        distDir
    });
    // TODO: Implement
    let clientRouterFilters;
    if (nextConfig.experimental.clientRouterFilter) {
    // TODO this need to be set correctly for filesystem cache to work
    }
    const supportedBrowsers = getSupportedBrowsers(projectPath, dev);
    const currentNodeJsVersion = process.versions.node;
    const rootPath = ((_opts_nextConfig_turbopack = opts.nextConfig.turbopack) == null ? void 0 : _opts_nextConfig_turbopack.root) || opts.nextConfig.outputFileTracingRoot || projectPath;
    const project = await bindings.turbo.createProject({
        rootPath,
        projectPath: normalizePath(relative(rootPath, projectPath) || '.'),
        distDir,
        nextConfig: opts.nextConfig,
        watch: {
            enable: dev,
            pollIntervalMs: (_nextConfig_watchOptions = nextConfig.watchOptions) == null ? void 0 : _nextConfig_watchOptions.pollIntervalMs
        },
        dev,
        env: process.env,
        defineEnv: createDefineEnv({
            isTurbopack: true,
            clientRouterFilters,
            config: nextConfig,
            dev,
            distDir,
            projectPath,
            fetchCacheKeyPrefix: opts.nextConfig.experimental.fetchCacheKeyPrefix,
            hasRewrites,
            // TODO: Implement
            middlewareMatchers: undefined,
            rewrites: opts.fsChecker.rewrites
        }),
        buildId,
        encryptionKey,
        previewProps: opts.fsChecker.prerenderManifest.preview,
        browserslistQuery: supportedBrowsers.join(', '),
        noMangling: false,
        writeRoutesHashesManifest: false,
        currentNodeJsVersion
    }, {
        persistentCaching: isFileSystemCacheEnabledForDev(opts.nextConfig),
        memoryLimit: (_opts_nextConfig_experimental = opts.nextConfig.experimental) == null ? void 0 : _opts_nextConfig_experimental.turbopackMemoryLimit,
        isShortSession: false
    });
    backgroundLogCompilationEvents(project, {
        eventTypes: [
            'StartupCacheInvalidationEvent',
            'TimingEvent'
        ]
    });
    setBundlerFindSourceMapImplementation(getSourceMapFromTurbopack.bind(null, project, projectPath));
    opts.onDevServerCleanup == null ? void 0 : opts.onDevServerCleanup.call(opts, async ()=>{
        setBundlerFindSourceMapImplementation(()=>undefined);
        await project.onExit();
        await (lockfile == null ? void 0 : lockfile.unlock());
    });
    const entrypointsSubscription = project.entrypointsSubscribe();
    const currentWrittenEntrypoints = new Map();
    const currentEntrypoints = {
        global: {
            app: undefined,
            document: undefined,
            error: undefined,
            middleware: undefined,
            instrumentation: undefined
        },
        page: new Map(),
        app: new Map()
    };
    const currentTopLevelIssues = new Map();
    const currentEntryIssues = new Map();
    const manifestLoader = new TurbopackManifestLoader({
        buildId,
        distDir,
        encryptionKey
    });
    // Dev specific
    const changeSubscriptions = new Map();
    const serverPathState = new Map();
    const readyIds = new Set();
    let currentEntriesHandlingResolve;
    let currentEntriesHandling = new Promise((resolve)=>currentEntriesHandlingResolve = resolve);
    const assetMapper = new AssetMapper();
    function clearRequireCache(key, writtenEndpoint, { force } = {}) {
        if (force) {
            for (const { path, contentHash } of writtenEndpoint.serverPaths){
                // We ignore source maps
                if (path.endsWith('.map')) continue;
                const localKey = `${key}:${path}`;
                serverPathState.set(localKey, contentHash);
                serverPathState.set(path, contentHash);
            }
        } else {
            // Figure out if the server files have changed
            let hasChange = false;
            for (const { path, contentHash } of writtenEndpoint.serverPaths){
                // We ignore source maps
                if (path.endsWith('.map')) continue;
                const localKey = `${key}:${path}`;
                const localHash = serverPathState.get(localKey);
                const globalHash = serverPathState.get(path);
                if (localHash && localHash !== contentHash || globalHash && globalHash !== contentHash) {
                    hasChange = true;
                    serverPathState.set(localKey, contentHash);
                    serverPathState.set(path, contentHash);
                } else {
                    if (!localHash) {
                        serverPathState.set(localKey, contentHash);
                    }
                    if (!globalHash) {
                        serverPathState.set(path, contentHash);
                    }
                }
            }
            if (!hasChange) {
                return false;
            }
        }
        resetFetch();
        // Not available in:
        // - Pages Router (no server-side HMR)
        // - Edge Runtime (uses browser runtime which already disposes chunks individually)
        if (typeof __next__clear_chunk_cache__ === 'function') {
            __next__clear_chunk_cache__();
        }
        const serverPaths = writtenEndpoint.serverPaths.map(({ path: p })=>join(distDir, p));
        for (const file of serverPaths){
            clearModuleContext(file);
            deleteCache(file);
        }
        return true;
    }
    const buildingIds = new Set();
    const startBuilding = (id, requestUrl, forceRebuild)=>{
        if (!forceRebuild && readyIds.has(id)) {
            return ()=>{};
        }
        if (buildingIds.size === 0) {
            consoleStore.setState({
                loading: true,
                trigger: id,
                url: requestUrl
            }, true);
        }
        buildingIds.add(id);
        return function finishBuilding() {
            if (buildingIds.size === 0) {
                return;
            }
            readyIds.add(id);
            buildingIds.delete(id);
            if (buildingIds.size === 0) {
                hmrEventHappened = false;
                consoleStore.setState({
                    loading: false
                }, true);
            }
        };
    };
    let hmrEventHappened = false;
    let hmrHash = 0;
    const clientsWithoutHtmlRequestId = new Set();
    const clientsByHtmlRequestId = new Map();
    const cacheStatusesByHtmlRequestId = new Map();
    const clientStates = new WeakMap();
    function sendToClient(client, message) {
        const data = typeof message.type === 'number' ? createBinaryHmrMessageData(message) : JSON.stringify(message);
        client.send(data);
    }
    function sendEnqueuedMessages() {
        for (const [, issueMap] of currentEntryIssues){
            if ([
                ...issueMap.values()
            ].filter((i)=>i.severity !== 'warning').length > 0) {
                // During compilation errors we want to delay the HMR events until errors are fixed
                return;
            }
        }
        for (const client of [
            ...clientsWithoutHtmlRequestId,
            ...clientsByHtmlRequestId.values()
        ]){
            const state = clientStates.get(client);
            if (!state) {
                continue;
            }
            for (const [, issueMap] of state.clientIssues){
                if ([
                    ...issueMap.values()
                ].filter((i)=>i.severity !== 'warning').length > 0) {
                    // During compilation errors we want to delay the HMR events until errors are fixed
                    return;
                }
            }
            for (const message of state.messages.values()){
                sendToClient(client, message);
            }
            state.messages.clear();
            if (state.turbopackUpdates.length > 0) {
                sendToClient(client, {
                    type: HMR_MESSAGE_SENT_TO_BROWSER.TURBOPACK_MESSAGE,
                    data: state.turbopackUpdates
                });
                state.turbopackUpdates.length = 0;
            }
        }
    }
    const sendEnqueuedMessagesDebounce = debounce(sendEnqueuedMessages, 2);
    const sendHmr = (id, message)=>{
        for (const client of [
            ...clientsWithoutHtmlRequestId,
            ...clientsByHtmlRequestId.values()
        ]){
            var _clientStates_get;
            (_clientStates_get = clientStates.get(client)) == null ? void 0 : _clientStates_get.messages.set(id, message);
        }
        hmrEventHappened = true;
        sendEnqueuedMessagesDebounce();
    };
    function sendTurbopackMessage(payload) {
        // TODO(PACK-2049): For some reason we end up emitting hundreds of issues messages on bigger apps,
        //   a lot of which are duplicates.
        //   They are currently not handled on the client at all, so might as well not send them for now.
        payload.diagnostics = [];
        payload.issues = [];
        for (const client of [
            ...clientsWithoutHtmlRequestId,
            ...clientsByHtmlRequestId.values()
        ]){
            var _clientStates_get;
            (_clientStates_get = clientStates.get(client)) == null ? void 0 : _clientStates_get.turbopackUpdates.push(payload);
        }
        hmrEventHappened = true;
        sendEnqueuedMessagesDebounce();
    }
    async function subscribeToChanges(key, includeIssues, endpoint, createMessage, onError) {
        if (changeSubscriptions.has(key)) {
            return;
        }
        const { side } = splitEntryKey(key);
        const changedPromise = endpoint[`${side}Changed`](includeIssues);
        changeSubscriptions.set(key, changedPromise);
        try {
            const changed = await changedPromise;
            for await (const change of changed){
                processIssues(currentEntryIssues, key, change, false, true);
                // TODO: Get an actual content hash from Turbopack.
                const message = await createMessage(change, String(++hmrHash));
                if (message) {
                    sendHmr(key, message);
                }
            }
        } catch (e) {
            changeSubscriptions.delete(key);
            const payload = await (onError == null ? void 0 : onError(e));
            if (payload) {
                sendHmr(key, payload);
            }
            return;
        }
        changeSubscriptions.delete(key);
    }
    async function unsubscribeFromChanges(key) {
        const subscription = await changeSubscriptions.get(key);
        if (subscription) {
            await (subscription.return == null ? void 0 : subscription.return.call(subscription));
            changeSubscriptions.delete(key);
        }
        currentEntryIssues.delete(key);
    }
    async function subscribeToHmrEvents(client, id) {
        const key = getEntryKey('assets', 'client', id);
        if (!hasEntrypointForKey(currentEntrypoints, key, assetMapper)) {
            // maybe throw an error / force the client to reload?
            return;
        }
        const state = clientStates.get(client);
        if (!state || state.subscriptions.has(id)) {
            return;
        }
        const subscription = project.hmrEvents(id);
        state.subscriptions.set(id, subscription);
        // The subscription will always emit once, which is the initial
        // computation. This is not a change, so swallow it.
        try {
            await subscription.next();
            for await (const data of subscription){
                processIssues(state.clientIssues, key, data, false, true);
                if (data.type !== 'issues') {
                    sendTurbopackMessage(data);
                }
            }
        } catch (e) {
            // The client might be using an HMR session from a previous server, tell them
            // to fully reload the page to resolve the issue. We can't use
            // `hotReloader.send` since that would force every connected client to
            // reload, only this client is out of date.
            const reloadMessage = {
                type: HMR_MESSAGE_SENT_TO_BROWSER.RELOAD_PAGE,
                data: `error in HMR event subscription for ${id}: ${e}`
            };
            sendToClient(client, reloadMessage);
            client.close();
            return;
        }
    }
    function unsubscribeFromHmrEvents(client, id) {
        const state = clientStates.get(client);
        if (!state) {
            return;
        }
        const subscription = state.subscriptions.get(id);
        subscription == null ? void 0 : subscription.return();
        const key = getEntryKey('assets', 'client', id);
        state.clientIssues.delete(key);
    }
    async function handleEntrypointsSubscription() {
        for await (const entrypoints of entrypointsSubscription){
            if (!currentEntriesHandlingResolve) {
                currentEntriesHandling = new Promise(// eslint-disable-next-line no-loop-func
                (resolve)=>currentEntriesHandlingResolve = resolve);
            }
            // Always process issues/diagnostics, even if there are no entrypoints yet
            processTopLevelIssues(currentTopLevelIssues, entrypoints);
            // Certain crtical issues prevent any entrypoints from being constructed so return early
            if (!('routes' in entrypoints)) {
                printBuildErrors(entrypoints, true);
                currentEntriesHandlingResolve();
                currentEntriesHandlingResolve = undefined;
                continue;
            }
            const routes = entrypoints.routes;
            const existingRoutes = [
                ...currentEntrypoints.app.keys(),
                ...currentEntrypoints.page.keys()
            ];
            const newRoutes = [
                ...routes.keys()
            ];
            const addedRoutes = newRoutes.filter((route)=>!currentEntrypoints.app.has(route) && !currentEntrypoints.page.has(route));
            const removedRoutes = existingRoutes.filter((route)=>!routes.has(route));
            await handleEntrypoints({
                entrypoints: entrypoints,
                currentEntrypoints,
                currentEntryIssues,
                manifestLoader,
                devRewrites: opts.fsChecker.rewrites,
                productionRewrites: undefined,
                logErrors: true,
                dev: {
                    assetMapper,
                    changeSubscriptions,
                    clients: [
                        ...clientsWithoutHtmlRequestId,
                        ...clientsByHtmlRequestId.values()
                    ],
                    clientStates,
                    serverFields,
                    hooks: {
                        handleWrittenEndpoint: (id, result, forceDeleteCache)=>{
                            currentWrittenEntrypoints.set(id, result);
                            return clearRequireCache(id, result, {
                                force: forceDeleteCache
                            });
                        },
                        propagateServerField: propagateServerField.bind(null, opts),
                        sendHmr,
                        startBuilding,
                        subscribeToChanges,
                        unsubscribeFromChanges,
                        unsubscribeFromHmrEvents
                    }
                }
            });
            // Reload matchers when the files have been compiled
            await propagateServerField(opts, 'reloadMatchers', undefined);
            if (addedRoutes.length > 0 || removedRoutes.length > 0) {
                // When the list of routes changes a new manifest should be fetched for Pages Router.
                hotReloader.send({
                    type: HMR_MESSAGE_SENT_TO_BROWSER.DEV_PAGES_MANIFEST_UPDATE,
                    data: [
                        {
                            devPagesManifest: true
                        }
                    ]
                });
            }
            for (const route of addedRoutes){
                hotReloader.send({
                    type: HMR_MESSAGE_SENT_TO_BROWSER.ADDED_PAGE,
                    data: [
                        route
                    ]
                });
            }
            for (const route of removedRoutes){
                hotReloader.send({
                    type: HMR_MESSAGE_SENT_TO_BROWSER.REMOVED_PAGE,
                    data: [
                        route
                    ]
                });
            }
            currentEntriesHandlingResolve();
            currentEntriesHandlingResolve = undefined;
        }
    }
    await mkdir(join(distDir, 'server'), {
        recursive: true
    });
    await mkdir(join(distDir, 'static', buildId), {
        recursive: true
    });
    await writeFile(join(distDir, 'package.json'), JSON.stringify({
        type: 'commonjs'
    }, null, 2));
    const middlewares = [
        getOverlayMiddleware({
            project,
            projectPath,
            isSrcDir: opts.isSrcDir
        }),
        getSourceMapMiddleware(project),
        getNextErrorFeedbackMiddleware(opts.telemetry),
        getDevOverlayFontMiddleware(),
        getDisableDevIndicatorMiddleware(),
        getRestartDevServerMiddleware({
            telemetry: opts.telemetry,
            turbopackProject: project
        }),
        devToolsConfigMiddleware({
            distDir,
            sendUpdateSignal: (data)=>{
                hotReloader.send({
                    type: HMR_MESSAGE_SENT_TO_BROWSER.DEVTOOLS_CONFIG,
                    data
                });
            }
        }),
        getAttachNodejsDebuggerMiddleware(),
        ...nextConfig.experimental.mcpServer ? [
            getMcpMiddleware({
                projectPath,
                distDir,
                nextConfig,
                pagesDir: opts.pagesDir,
                appDir: opts.appDir,
                sendHmrMessage: (message)=>hotReloader.send(message),
                getActiveConnectionCount: ()=>clientsWithoutHtmlRequestId.size + clientsByHtmlRequestId.size,
                getDevServerUrl: ()=>process.env.__NEXT_PRIVATE_ORIGIN
            })
        ] : []
    ];
    setStackFrameResolver(async (request)=>{
        return getOriginalStackFrames({
            project,
            projectPath,
            isServer: request.isServer,
            isEdgeServer: request.isEdgeServer,
            isAppDirectory: request.isAppDirectory,
            frames: request.frames
        });
    });
    let versionInfoCached;
    // This fetch, even though not awaited, is not kicked off eagerly because the first `fetch()` in
    // Node.js adds roughly 20ms main-thread blocking to load the SSL certificate cache
    // We don't want that blocking time to be in the hot path for the `ready in` logging.
    // Instead, the fetch is kicked off lazily when the first `getVersionInfoCached()` is called.
    const getVersionInfoCached = ()=>{
        if (!versionInfoCached) {
            versionInfoCached = getVersionInfo();
        }
        return versionInfoCached;
    };
    let devtoolsFrontendUrl;
    const inspectorURLRaw = inspector.url();
    if (inspectorURLRaw !== undefined) {
        const inspectorURL = new URL(inspectorURLRaw);
        let debugInfo;
        try {
            const debugInfoList = await fetch(`http://${inspectorURL.host}/json/list`).then((res)=>res.json());
            debugInfo = debugInfoList[0];
        } catch  {}
        if (debugInfo) {
            devtoolsFrontendUrl = debugInfo.devtoolsFrontendUrl;
        }
    }
    const hotReloader = {
        turbopackProject: project,
        activeWebpackConfigs: undefined,
        serverStats: null,
        edgeServerStats: null,
        async run (req, res, _parsedUrl) {
            var _req_url;
            // intercept page chunks request and ensure them with turbopack
            if ((_req_url = req.url) == null ? void 0 : _req_url.startsWith('/_next/static/chunks/pages/')) {
                const params = matchNextPageBundleRequest(req.url);
                if (params) {
                    const decodedPagePath = `/${params.path.map((param)=>decodeURIComponent(param)).join('/')}`;
                    const denormalizedPagePath = denormalizePagePath(decodedPagePath);
                    await hotReloader.ensurePage({
                        page: denormalizedPagePath,
                        clientOnly: false,
                        definition: undefined,
                        url: req.url
                    }).catch(console.error);
                }
            }
            for (const middleware of middlewares){
                let calledNext = false;
                await middleware(req, res, ()=>{
                    calledNext = true;
                });
                if (!calledNext) {
                    return {
                        finished: true
                    };
                }
            }
            // Request was not finished.
            return {
                finished: undefined
            };
        },
        // TODO: Figure out if socket type can match the NextJsHotReloaderInterface
        onHMR (req, socket, head, onUpgrade) {
            wsServer.handleUpgrade(req, socket, head, (client)=>{
                const clientIssues = new Map();
                const subscriptions = new Map();
                const htmlRequestId = req.url ? new URL(req.url, 'http://n').searchParams.get('id') : null;
                // Clients with a request ID are inferred App Router clients. If Cache
                // Components is not enabled, we consider those legacy clients. Pages
                // Router clients are also considered legacy clients. TODO: Maybe mark
                // clients as App Router / Pages Router clients explicitly, instead of
                // inferring it from the presence of a request ID.
                if (htmlRequestId) {
                    clientsByHtmlRequestId.set(htmlRequestId, client);
                    const enableCacheComponents = nextConfig.cacheComponents;
                    if (enableCacheComponents) {
                        onUpgrade(client, {
                            isLegacyClient: false
                        });
                        const cacheStatus = cacheStatusesByHtmlRequestId.get(htmlRequestId);
                        if (cacheStatus !== undefined) {
                            sendToClient(client, {
                                type: HMR_MESSAGE_SENT_TO_BROWSER.CACHE_INDICATOR,
                                state: cacheStatus
                            });
                            cacheStatusesByHtmlRequestId.delete(htmlRequestId);
                        }
                    } else {
                        onUpgrade(client, {
                            isLegacyClient: true
                        });
                    }
                    connectReactDebugChannelForHtmlRequest(htmlRequestId, sendToClient.bind(null, client));
                    sendSerializedErrorsToClientForHtmlRequest(htmlRequestId, sendToClient.bind(null, client));
                } else {
                    clientsWithoutHtmlRequestId.add(client);
                    onUpgrade(client, {
                        isLegacyClient: true
                    });
                }
                clientStates.set(client, {
                    clientIssues,
                    messages: new Map(),
                    turbopackUpdates: [],
                    subscriptions
                });
                client.on('close', ()=>{
                    // Remove active subscriptions
                    for (const subscription of subscriptions.values()){
                        subscription.return == null ? void 0 : subscription.return.call(subscription);
                    }
                    clientStates.delete(client);
                    if (htmlRequestId) {
                        clientsByHtmlRequestId.delete(htmlRequestId);
                        deleteReactDebugChannelForHtmlRequest(htmlRequestId);
                    } else {
                        clientsWithoutHtmlRequestId.delete(client);
                    }
                });
                client.addEventListener('message', async ({ data })=>{
                    const parsedData = JSON.parse(typeof data !== 'string' ? data.toString() : data);
                    // Next.js messages
                    switch(parsedData.event){
                        case 'span-end':
                            {
                                hotReloaderSpan.manualTraceChild(parsedData.spanName, msToNs(parsedData.startTime), msToNs(parsedData.endTime), parsedData.attributes);
                                break;
                            }
                        case 'client-hmr-latency':
                            hotReloaderSpan.manualTraceChild(parsedData.event, msToNs(parsedData.startTime), msToNs(parsedData.endTime), {
                                updatedModules: parsedData.updatedModules,
                                page: parsedData.page,
                                isPageHidden: parsedData.isPageHidden
                            });
                            break;
                        case 'client-error':
                        case 'client-warning':
                        case 'client-success':
                        case 'server-component-reload-page':
                        case 'client-reload-page':
                        case 'client-removed-page':
                        case 'client-full-reload':
                            const { hadRuntimeError, dependencyChain } = parsedData;
                            if (hadRuntimeError) {
                                Log.warn(FAST_REFRESH_RUNTIME_RELOAD);
                            }
                            if (Array.isArray(dependencyChain) && typeof dependencyChain[0] === 'string') {
                                const cleanedModulePath = dependencyChain[0].replace(/^\[project\]/, '.').replace(/ \[.*\] \(.*\)$/, '');
                                Log.warn(`Fast Refresh had to perform a full reload when ${cleanedModulePath} changed. Read more: https://nextjs.org/docs/messages/fast-refresh-reload`);
                            }
                            break;
                        case 'client-added-page':
                            break;
                        case 'browser-logs':
                            {
                                if (nextConfig.experimental.browserDebugInfoInTerminal) {
                                    await receiveBrowserLogsTurbopack({
                                        entries: parsedData.entries,
                                        router: parsedData.router,
                                        sourceType: parsedData.sourceType,
                                        project,
                                        projectPath,
                                        distDir,
                                        config: nextConfig.experimental.browserDebugInfoInTerminal
                                    });
                                }
                                break;
                            }
                        case 'client-file-logs':
                            {
                                // Always log to file regardless of terminal flag
                                await handleClientFileLogs(parsedData.logs);
                                break;
                            }
                        case 'ping':
                            {
                                break;
                            }
                        case 'mcp-error-state-response':
                            {
                                handleErrorStateResponse(parsedData.requestId, parsedData.errorState, parsedData.url);
                                break;
                            }
                        case 'mcp-page-metadata-response':
                            {
                                handlePageMetadataResponse(parsedData.requestId, parsedData.segmentTrieData, parsedData.url);
                                break;
                            }
                        default:
                            // Might be a Turbopack message...
                            if (!parsedData.type) {
                                throw Object.defineProperty(new Error(`unrecognized HMR message "${data}"`), "__NEXT_ERROR_CODE", {
                                    value: "E155",
                                    enumerable: false,
                                    configurable: true
                                });
                            }
                    }
                    // Turbopack messages
                    switch(parsedData.type){
                        case 'turbopack-subscribe':
                            subscribeToHmrEvents(client, parsedData.path);
                            break;
                        case 'turbopack-unsubscribe':
                            unsubscribeFromHmrEvents(client, parsedData.path);
                            break;
                        default:
                            if (!parsedData.event) {
                                throw Object.defineProperty(new Error(`unrecognized Turbopack HMR message "${data}"`), "__NEXT_ERROR_CODE", {
                                    value: "E492",
                                    enumerable: false,
                                    configurable: true
                                });
                            }
                    }
                });
                const turbopackConnectedMessage = {
                    type: HMR_MESSAGE_SENT_TO_BROWSER.TURBOPACK_CONNECTED,
                    data: {
                        sessionId
                    }
                };
                sendToClient(client, turbopackConnectedMessage);
                const errors = [];
                for (const entryIssues of currentEntryIssues.values()){
                    for (const issue of entryIssues.values()){
                        if (issue.severity !== 'warning') {
                            errors.push({
                                message: formatIssue(issue)
                            });
                        } else {
                            printNonFatalIssue(issue);
                        }
                    }
                }
                if (devIndicatorServerState.disabledUntil < Date.now()) {
                    devIndicatorServerState.disabledUntil = 0;
                }
                ;
                (async function() {
                    const versionInfo = await getVersionInfoCached();
                    const devToolsConfig = await getDevToolsConfig(distDir);
                    const syncMessage = {
                        type: HMR_MESSAGE_SENT_TO_BROWSER.SYNC,
                        errors,
                        warnings: [],
                        hash: '',
                        versionInfo,
                        debug: {
                            devtoolsFrontendUrl
                        },
                        devIndicator: devIndicatorServerState,
                        devToolsConfig
                    };
                    sendToClient(client, syncMessage);
                })();
            });
        },
        send (action) {
            const payload = JSON.stringify(action);
            for (const client of [
                ...clientsWithoutHtmlRequestId,
                ...clientsByHtmlRequestId.values()
            ]){
                client.send(payload);
            }
        },
        sendToLegacyClients (action) {
            const payload = JSON.stringify(action);
            // Clients with a request ID are inferred App Router clients. If Cache
            // Components is not enabled, we consider those legacy clients. Pages
            // Router clients are also considered legacy clients. TODO: Maybe mark
            // clients as App Router / Pages Router clients explicitly, instead of
            // inferring it from the presence of a request ID.
            if (!nextConfig.cacheComponents) {
                for (const client of clientsByHtmlRequestId.values()){
                    client.send(payload);
                }
            }
            for (const client of clientsWithoutHtmlRequestId){
                client.send(payload);
            }
        },
        setCacheStatus (status, htmlRequestId) {
            // Legacy clients don't have Cache Components.
            const client = clientsByHtmlRequestId.get(htmlRequestId);
            if (client !== undefined) {
                sendToClient(client, {
                    type: HMR_MESSAGE_SENT_TO_BROWSER.CACHE_INDICATOR,
                    state: status
                });
            } else {
                // If the client is not connected, store the status so that we can send it
                // when the client connects.
                cacheStatusesByHtmlRequestId.set(htmlRequestId, status);
            }
        },
        setReactDebugChannel (debugChannel, htmlRequestId, requestId) {
            const client = clientsByHtmlRequestId.get(htmlRequestId);
            if (htmlRequestId === requestId) {
                // The debug channel is for the HTML request.
                if (client) {
                    // If the client is connected, we can connect the debug channel for
                    // the HTML request immediately.
                    connectReactDebugChannel(htmlRequestId, debugChannel, sendToClient.bind(null, client));
                } else {
                    // Otherwise, we'll do that when the client connects and just store
                    // the debug channel.
                    setReactDebugChannelForHtmlRequest(htmlRequestId, debugChannel);
                }
            } else if (client) {
                // The debug channel is for a subsequent request (e.g. client-side
                // navigation for server function call). If the client is not connected
                // anymore, we don't need to connect the debug channel.
                connectReactDebugChannel(requestId, debugChannel, sendToClient.bind(null, client));
            }
        },
        sendErrorsToBrowser (errorsRscStream, htmlRequestId) {
            const client = clientsByHtmlRequestId.get(htmlRequestId);
            if (client) {
                // If the client is connected, we can send the errors immediately.
                sendSerializedErrorsToClient(errorsRscStream, sendToClient.bind(null, client));
            } else {
                // Otherwise, store the errors stream so that we can send it when the
                // client connects.
                setErrorsRscStreamForHtmlRequest(htmlRequestId, errorsRscStream);
            }
        },
        setHmrServerError (_error) {
        // Not implemented yet.
        },
        clearHmrServerError () {
        // Not implemented yet.
        },
        async start () {},
        async getCompilationErrors (page) {
            const appEntryKey = getEntryKey('app', 'server', page);
            const pagesEntryKey = getEntryKey('pages', 'server', page);
            const topLevelIssues = currentTopLevelIssues.values();
            const thisEntryIssues = currentEntryIssues.get(appEntryKey) ?? currentEntryIssues.get(pagesEntryKey);
            if (thisEntryIssues !== undefined && thisEntryIssues.size > 0) {
                // If there is an error related to the requesting page we display it instead of the first error
                return [
                    ...topLevelIssues,
                    ...thisEntryIssues.values()
                ].map((issue)=>{
                    const formattedIssue = formatIssue(issue);
                    if (issue.severity === 'warning') {
                        printNonFatalIssue(issue);
                        return null;
                    } else if (isWellKnownError(issue)) {
                        Log.error(formattedIssue);
                    }
                    return Object.defineProperty(new Error(formattedIssue), "__NEXT_ERROR_CODE", {
                        value: "E394",
                        enumerable: false,
                        configurable: true
                    });
                }).filter((error)=>error !== null);
            }
            // Otherwise, return all errors across pages
            const errors = [];
            for (const issue of topLevelIssues){
                if (issue.severity !== 'warning') {
                    errors.push(Object.defineProperty(new Error(formatIssue(issue)), "__NEXT_ERROR_CODE", {
                        value: "E394",
                        enumerable: false,
                        configurable: true
                    }));
                }
            }
            for (const entryIssues of currentEntryIssues.values()){
                for (const issue of entryIssues.values()){
                    if (issue.severity !== 'warning') {
                        const message = formatIssue(issue);
                        errors.push(Object.defineProperty(new Error(message), "__NEXT_ERROR_CODE", {
                            value: "E394",
                            enumerable: false,
                            configurable: true
                        }));
                    } else {
                        printNonFatalIssue(issue);
                    }
                }
            }
            return errors;
        },
        async invalidate ({ // .env files or tsconfig/jsconfig change
        reloadAfterInvalidation }) {
            if (reloadAfterInvalidation) {
                for (const [key, entrypoint] of currentWrittenEntrypoints){
                    clearRequireCache(key, entrypoint, {
                        force: true
                    });
                }
                await clearAllModuleContexts();
                this.send({
                    type: HMR_MESSAGE_SENT_TO_BROWSER.SERVER_COMPONENT_CHANGES,
                    hash: String(++hmrHash)
                });
            }
        },
        async buildFallbackError () {
        // Not implemented yet.
        },
        async ensurePage ({ page: inputPage, // Unused parameters
        // clientOnly,
        appPaths, definition, isApp, url: requestUrl }) {
            // When there is no route definition this is an internal file not a route the user added.
            // Middleware and instrumentation are handled in turbpack-utils.ts handleEntrypoints instead.
            if (!definition) {
                if (inputPage === '/middleware') return;
                if (inputPage === '/src/middleware') return;
                if (inputPage === '/instrumentation') return;
                if (inputPage === '/src/instrumentation') return;
            }
            return hotReloaderSpan.traceChild('ensure-page', {
                inputPage
            }).traceAsyncFn(async ()=>{
                if (BLOCKED_PAGES.includes(inputPage) && inputPage !== '/_error') {
                    return;
                }
                await currentEntriesHandling;
                // TODO We shouldn't look into the filesystem again. This should use the information from entrypoints
                let routeDef = definition ?? await findPagePathData(projectPath, inputPage, nextConfig.pageExtensions, opts.pagesDir, opts.appDir, !!nextConfig.experimental.globalNotFound);
                // If the route is actually an app page route, then we should have access
                // to the app route definition, and therefore, the appPaths from it.
                if (!appPaths && definition && isAppPageRouteDefinition(definition)) {
                    appPaths = definition.appPaths;
                }
                let page = routeDef.page;
                if (appPaths) {
                    const normalizedPage = normalizeAppPath(page);
                    // filter out paths that are not exact matches (e.g. catchall)
                    const matchingAppPaths = appPaths.filter((path)=>normalizeAppPath(path) === normalizedPage);
                    // the last item in the array is the root page, if there are parallel routes
                    page = matchingAppPaths[matchingAppPaths.length - 1];
                }
                const pathname = (definition == null ? void 0 : definition.pathname) ?? inputPage;
                if (page === '/_error') {
                    let finishBuilding = startBuilding(pathname, requestUrl, false);
                    try {
                        await handlePagesErrorRoute({
                            currentEntryIssues,
                            entrypoints: currentEntrypoints,
                            manifestLoader,
                            devRewrites: opts.fsChecker.rewrites,
                            productionRewrites: undefined,
                            logErrors: true,
                            hooks: {
                                subscribeToChanges,
                                handleWrittenEndpoint: (id, result, forceDeleteCache)=>{
                                    currentWrittenEntrypoints.set(id, result);
                                    assetMapper.setPathsForKey(id, result.clientPaths);
                                    return clearRequireCache(id, result, {
                                        force: forceDeleteCache
                                    });
                                }
                            }
                        });
                    } finally{
                        finishBuilding();
                    }
                    return;
                }
                const isInsideAppDir = routeDef.bundlePath.startsWith('app/');
                const isEntryMetadataRouteFile = isMetadataRouteFile(routeDef.filename.replace(opts.appDir || '', ''), nextConfig.pageExtensions, true);
                const normalizedAppPage = isEntryMetadataRouteFile ? normalizedPageToTurbopackStructureRoute(page, extname(routeDef.filename)) : page;
                const route = isInsideAppDir ? currentEntrypoints.app.get(normalizedAppPage) : currentEntrypoints.page.get(page);
                if (!route) {
                    // TODO: why is this entry missing in turbopack?
                    if (page === '/middleware') return;
                    if (page === '/src/middleware') return;
                    if (page === '/proxy') return;
                    if (page === '/src/proxy') return;
                    if (page === '/instrumentation') return;
                    if (page === '/src/instrumentation') return;
                    throw new PageNotFoundError(`route not found ${page}`);
                }
                // We don't throw on ensureOpts.isApp === true for page-api
                // since this can happen when app pages make
                // api requests to page API routes.
                if (isApp && route.type === 'page') {
                    throw Object.defineProperty(new Error(`mis-matched route type: isApp && page for ${page}`), "__NEXT_ERROR_CODE", {
                        value: "E373",
                        enumerable: false,
                        configurable: true
                    });
                }
                const finishBuilding = startBuilding(pathname, requestUrl, false);
                try {
                    await handleRouteType({
                        dev,
                        page,
                        pathname,
                        route,
                        currentEntryIssues,
                        entrypoints: currentEntrypoints,
                        manifestLoader,
                        readyIds,
                        devRewrites: opts.fsChecker.rewrites,
                        productionRewrites: undefined,
                        logErrors: true,
                        hooks: {
                            subscribeToChanges,
                            handleWrittenEndpoint: (id, result, forceDeleteCache)=>{
                                currentWrittenEntrypoints.set(id, result);
                                assetMapper.setPathsForKey(id, result.clientPaths);
                                return clearRequireCache(id, result, {
                                    force: forceDeleteCache
                                });
                            }
                        }
                    });
                } finally{
                    finishBuilding();
                }
            });
        },
        close () {
            // Report MCP telemetry if MCP server is enabled
            recordMcpTelemetry(opts.telemetry);
            for (const wsClient of [
                ...clientsWithoutHtmlRequestId,
                ...clientsByHtmlRequestId.values()
            ]){
                // it's okay to not cleanly close these websocket connections, this is dev
                wsClient.terminate();
            }
            clientsWithoutHtmlRequestId.clear();
            clientsByHtmlRequestId.clear();
        }
    };
    handleEntrypointsSubscription().catch((err)=>{
        console.error(err);
        process.exit(1);
    });
    // Write empty manifests
    await currentEntriesHandling;
    await manifestLoader.writeManifests({
        devRewrites: opts.fsChecker.rewrites,
        productionRewrites: undefined,
        entrypoints: currentEntrypoints
    });
    async function handleProjectUpdates() {
        for await (const updateMessage of project.updateInfoSubscribe(30)){
            switch(updateMessage.updateType){
                case 'start':
                    {
                        hotReloader.send({
                            type: HMR_MESSAGE_SENT_TO_BROWSER.BUILDING
                        });
                        break;
                    }
                case 'end':
                    {
                        sendEnqueuedMessages();
                        function addToErrorsMap(errorsMap, issueMap) {
                            for (const [key, issue] of issueMap){
                                if (issue.severity === 'warning') continue;
                                if (errorsMap.has(key)) continue;
                                const message = formatIssue(issue);
                                errorsMap.set(key, {
                                    message,
                                    details: issue.detail ? renderStyledStringToErrorAnsi(issue.detail) : undefined
                                });
                            }
                        }
                        function addErrors(errorsMap, issues) {
                            for (const issueMap of issues.values()){
                                addToErrorsMap(errorsMap, issueMap);
                            }
                        }
                        const errors = new Map();
                        addToErrorsMap(errors, currentTopLevelIssues);
                        addErrors(errors, currentEntryIssues);
                        for (const client of [
                            ...clientsWithoutHtmlRequestId,
                            ...clientsByHtmlRequestId.values()
                        ]){
                            const state = clientStates.get(client);
                            if (!state) {
                                continue;
                            }
                            const clientErrors = new Map(errors);
                            addErrors(clientErrors, state.clientIssues);
                            sendToClient(client, {
                                type: HMR_MESSAGE_SENT_TO_BROWSER.BUILT,
                                hash: String(++hmrHash),
                                errors: [
                                    ...clientErrors.values()
                                ],
                                warnings: []
                            });
                        }
                        if (hmrEventHappened) {
                            const time = updateMessage.value.duration;
                            const timeMessage = time > 2000 ? `${Math.round(time / 100) / 10}s` : `${time}ms`;
                            Log.event(`Compiled in ${timeMessage}`);
                            hmrEventHappened = false;
                        }
                        break;
                    }
                default:
            }
        }
    }
    handleProjectUpdates().catch((err)=>{
        console.error(err);
        process.exit(1);
    });
    return hotReloader;
}

//# sourceMappingURL=hot-reloader-turbopack.js.map