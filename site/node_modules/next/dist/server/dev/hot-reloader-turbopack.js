"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "createHotReloaderTurbopack", {
    enumerable: true,
    get: function() {
        return createHotReloaderTurbopack;
    }
});
const _promises = require("fs/promises");
const _inspector = /*#__PURE__*/ _interop_require_wildcard(require("inspector"));
const _path = require("path");
const _url = require("url");
const _ws = /*#__PURE__*/ _interop_require_default(require("next/dist/compiled/ws"));
const _store = require("../../build/output/store");
const _hotreloadertypes = require("./hot-reloader-types");
const _swc = require("../../build/swc");
const _log = /*#__PURE__*/ _interop_require_wildcard(require("../../build/output/log"));
const _constants = require("../../shared/lib/constants");
const _middlewareturbopack = require("./middleware-turbopack");
const _utils = require("../../shared/lib/utils");
const _utils1 = require("../utils");
const _requirecache = require("./require-cache");
const _renderserver = require("../lib/render-server");
const _denormalizepagepath = require("../../shared/lib/page-path/denormalize-page-path");
const _trace = require("../../trace");
const _turbopackutils = require("./turbopack-utils");
const _setupdevbundler = require("../lib/router-utils/setup-dev-bundler");
const _manifestloader = require("../../shared/lib/turbopack/manifest-loader");
const _ondemandentryhandler = require("./on-demand-entry-handler");
const _entrykey = require("../../shared/lib/turbopack/entry-key");
const _messages = require("./messages");
const _encryptionutilsserver = require("../app-render/encryption-utils-server");
const _apppageroutedefinition = require("../route-definitions/app-page-route-definition");
const _apppaths = require("../../shared/lib/router/utils/app-paths");
const _ismetadataroute = require("../../lib/metadata/is-metadata-route");
const _patcherrorinspect = require("../patch-error-inspect");
const _getnexterrorfeedbackmiddleware = require("../../next-devtools/server/get-next-error-feedback-middleware");
const _utils2 = require("../../shared/lib/turbopack/utils");
const _getdevoverlayfontmiddleware = require("../../next-devtools/server/font/get-dev-overlay-font-middleware");
const _devindicatorserverstate = require("./dev-indicator-server-state");
const _devindicatormiddleware = require("../../next-devtools/server/dev-indicator-middleware");
const _restartdevservermiddleware = require("../../next-devtools/server/restart-dev-server-middleware");
const _compilationevents = require("../../shared/lib/turbopack/compilation-events");
const _utils3 = require("../../build/utils");
const _receivelogs = require("./browser-logs/receive-logs");
const _normalizepath = require("../../lib/normalize-path");
const _devtoolsconfigmiddleware = require("../../next-devtools/server/devtools-config-middleware");
const _attachnodejsdebuggermiddleware = require("../../next-devtools/server/attach-nodejs-debugger-middleware");
const _debugchannel = require("./debug-channel");
const _hotreloadersharedutils = require("./hot-reloader-shared-utils");
const _getmcpmiddleware = require("../mcp/get-mcp-middleware");
const _geterrors = require("../mcp/tools/get-errors");
const _getpagemetadata = require("../mcp/tools/get-page-metadata");
const _formaterrors = require("../mcp/tools/utils/format-errors");
const _mcptelemetrytracker = require("../mcp/mcp-telemetry-tracker");
const _filelogger = require("./browser-logs/file-logger");
const _serializederrors = require("./serialized-errors");
function _interop_require_default(obj) {
    return obj && obj.__esModule ? obj : {
        default: obj
    };
}
function _getRequireWildcardCache(nodeInterop) {
    if (typeof WeakMap !== "function") return null;
    var cacheBabelInterop = new WeakMap();
    var cacheNodeInterop = new WeakMap();
    return (_getRequireWildcardCache = function(nodeInterop) {
        return nodeInterop ? cacheNodeInterop : cacheBabelInterop;
    })(nodeInterop);
}
function _interop_require_wildcard(obj, nodeInterop) {
    if (!nodeInterop && obj && obj.__esModule) {
        return obj;
    }
    if (obj === null || typeof obj !== "object" && typeof obj !== "function") {
        return {
            default: obj
        };
    }
    var cache = _getRequireWildcardCache(nodeInterop);
    if (cache && cache.has(obj)) {
        return cache.get(obj);
    }
    var newObj = {
        __proto__: null
    };
    var hasPropertyDescriptor = Object.defineProperty && Object.getOwnPropertyDescriptor;
    for(var key in obj){
        if (key !== "default" && Object.prototype.hasOwnProperty.call(obj, key)) {
            var desc = hasPropertyDescriptor ? Object.getOwnPropertyDescriptor(obj, key) : null;
            if (desc && (desc.get || desc.set)) {
                Object.defineProperty(newObj, key, desc);
            } else {
                newObj[key] = obj[key];
            }
        }
    }
    newObj.default = obj;
    if (cache) {
        cache.set(obj, newObj);
    }
    return newObj;
}
const wsServer = new _ws.default.Server({
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
            sourceMap.sources[i] = (0, _url.pathToFileURL)((0, _path.join)(projectRoot, sourceMap.sources[i].replace(/turbopack:\/\/\/\[project\]/, ''))).toString();
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
async function createHotReloaderTurbopack(opts, serverFields, distDir, resetFetch, lockfile) {
    var _opts_nextConfig_turbopack, _nextConfig_watchOptions, _opts_nextConfig_experimental;
    const dev = true;
    const buildId = 'development';
    const { nextConfig, dir: projectPath } = opts;
    const bindings = (0, _swc.getBindingsSync)();
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
    const hotReloaderSpan = (0, _trace.trace)('hot-reloader', undefined, {
        version: "16.1.6"
    });
    // Ensure the hotReloaderSpan is flushed immediately as it's the parentSpan for all processing
    // of the current `next dev` invocation.
    hotReloaderSpan.stop();
    // Initialize log monitor for file logging
    // Enable logging by default in development mode
    const mcpServerEnabled = !!nextConfig.experimental.mcpServer;
    const fileLogger = (0, _filelogger.getFileLogger)();
    fileLogger.initialize(distDir, mcpServerEnabled);
    const encryptionKey = await (0, _encryptionutilsserver.generateEncryptionKeyBase64)({
        isBuild: false,
        distDir
    });
    // TODO: Implement
    let clientRouterFilters;
    if (nextConfig.experimental.clientRouterFilter) {
    // TODO this need to be set correctly for filesystem cache to work
    }
    const supportedBrowsers = (0, _utils3.getSupportedBrowsers)(projectPath, dev);
    const currentNodeJsVersion = process.versions.node;
    const rootPath = ((_opts_nextConfig_turbopack = opts.nextConfig.turbopack) == null ? void 0 : _opts_nextConfig_turbopack.root) || opts.nextConfig.outputFileTracingRoot || projectPath;
    const project = await bindings.turbo.createProject({
        rootPath,
        projectPath: (0, _normalizepath.normalizePath)((0, _path.relative)(rootPath, projectPath) || '.'),
        distDir,
        nextConfig: opts.nextConfig,
        watch: {
            enable: dev,
            pollIntervalMs: (_nextConfig_watchOptions = nextConfig.watchOptions) == null ? void 0 : _nextConfig_watchOptions.pollIntervalMs
        },
        dev,
        env: process.env,
        defineEnv: (0, _swc.createDefineEnv)({
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
        persistentCaching: (0, _utils2.isFileSystemCacheEnabledForDev)(opts.nextConfig),
        memoryLimit: (_opts_nextConfig_experimental = opts.nextConfig.experimental) == null ? void 0 : _opts_nextConfig_experimental.turbopackMemoryLimit,
        isShortSession: false
    });
    (0, _compilationevents.backgroundLogCompilationEvents)(project, {
        eventTypes: [
            'StartupCacheInvalidationEvent',
            'TimingEvent'
        ]
    });
    (0, _patcherrorinspect.setBundlerFindSourceMapImplementation)(getSourceMapFromTurbopack.bind(null, project, projectPath));
    opts.onDevServerCleanup == null ? void 0 : opts.onDevServerCleanup.call(opts, async ()=>{
        (0, _patcherrorinspect.setBundlerFindSourceMapImplementation)(()=>undefined);
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
    const manifestLoader = new _manifestloader.TurbopackManifestLoader({
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
    const assetMapper = new _turbopackutils.AssetMapper();
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
        const serverPaths = writtenEndpoint.serverPaths.map(({ path: p })=>(0, _path.join)(distDir, p));
        for (const file of serverPaths){
            (0, _renderserver.clearModuleContext)(file);
            (0, _requirecache.deleteCache)(file);
        }
        return true;
    }
    const buildingIds = new Set();
    const startBuilding = (id, requestUrl, forceRebuild)=>{
        if (!forceRebuild && readyIds.has(id)) {
            return ()=>{};
        }
        if (buildingIds.size === 0) {
            _store.store.setState({
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
                _store.store.setState({
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
        const data = typeof message.type === 'number' ? (0, _messages.createBinaryHmrMessageData)(message) : JSON.stringify(message);
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
                    type: _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.TURBOPACK_MESSAGE,
                    data: state.turbopackUpdates
                });
                state.turbopackUpdates.length = 0;
            }
        }
    }
    const sendEnqueuedMessagesDebounce = (0, _utils1.debounce)(sendEnqueuedMessages, 2);
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
        const { side } = (0, _entrykey.splitEntryKey)(key);
        const changedPromise = endpoint[`${side}Changed`](includeIssues);
        changeSubscriptions.set(key, changedPromise);
        try {
            const changed = await changedPromise;
            for await (const change of changed){
                (0, _utils2.processIssues)(currentEntryIssues, key, change, false, true);
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
        const key = (0, _entrykey.getEntryKey)('assets', 'client', id);
        if (!(0, _turbopackutils.hasEntrypointForKey)(currentEntrypoints, key, assetMapper)) {
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
                (0, _utils2.processIssues)(state.clientIssues, key, data, false, true);
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
                type: _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.RELOAD_PAGE,
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
        const key = (0, _entrykey.getEntryKey)('assets', 'client', id);
        state.clientIssues.delete(key);
    }
    async function handleEntrypointsSubscription() {
        for await (const entrypoints of entrypointsSubscription){
            if (!currentEntriesHandlingResolve) {
                currentEntriesHandling = new Promise(// eslint-disable-next-line no-loop-func
                (resolve)=>currentEntriesHandlingResolve = resolve);
            }
            // Always process issues/diagnostics, even if there are no entrypoints yet
            (0, _turbopackutils.processTopLevelIssues)(currentTopLevelIssues, entrypoints);
            // Certain crtical issues prevent any entrypoints from being constructed so return early
            if (!('routes' in entrypoints)) {
                (0, _utils3.printBuildErrors)(entrypoints, true);
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
            await (0, _turbopackutils.handleEntrypoints)({
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
                        propagateServerField: _setupdevbundler.propagateServerField.bind(null, opts),
                        sendHmr,
                        startBuilding,
                        subscribeToChanges,
                        unsubscribeFromChanges,
                        unsubscribeFromHmrEvents
                    }
                }
            });
            // Reload matchers when the files have been compiled
            await (0, _setupdevbundler.propagateServerField)(opts, 'reloadMatchers', undefined);
            if (addedRoutes.length > 0 || removedRoutes.length > 0) {
                // When the list of routes changes a new manifest should be fetched for Pages Router.
                hotReloader.send({
                    type: _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.DEV_PAGES_MANIFEST_UPDATE,
                    data: [
                        {
                            devPagesManifest: true
                        }
                    ]
                });
            }
            for (const route of addedRoutes){
                hotReloader.send({
                    type: _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.ADDED_PAGE,
                    data: [
                        route
                    ]
                });
            }
            for (const route of removedRoutes){
                hotReloader.send({
                    type: _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.REMOVED_PAGE,
                    data: [
                        route
                    ]
                });
            }
            currentEntriesHandlingResolve();
            currentEntriesHandlingResolve = undefined;
        }
    }
    await (0, _promises.mkdir)((0, _path.join)(distDir, 'server'), {
        recursive: true
    });
    await (0, _promises.mkdir)((0, _path.join)(distDir, 'static', buildId), {
        recursive: true
    });
    await (0, _promises.writeFile)((0, _path.join)(distDir, 'package.json'), JSON.stringify({
        type: 'commonjs'
    }, null, 2));
    const middlewares = [
        (0, _middlewareturbopack.getOverlayMiddleware)({
            project,
            projectPath,
            isSrcDir: opts.isSrcDir
        }),
        (0, _middlewareturbopack.getSourceMapMiddleware)(project),
        (0, _getnexterrorfeedbackmiddleware.getNextErrorFeedbackMiddleware)(opts.telemetry),
        (0, _getdevoverlayfontmiddleware.getDevOverlayFontMiddleware)(),
        (0, _devindicatormiddleware.getDisableDevIndicatorMiddleware)(),
        (0, _restartdevservermiddleware.getRestartDevServerMiddleware)({
            telemetry: opts.telemetry,
            turbopackProject: project
        }),
        (0, _devtoolsconfigmiddleware.devToolsConfigMiddleware)({
            distDir,
            sendUpdateSignal: (data)=>{
                hotReloader.send({
                    type: _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.DEVTOOLS_CONFIG,
                    data
                });
            }
        }),
        (0, _attachnodejsdebuggermiddleware.getAttachNodejsDebuggerMiddleware)(),
        ...nextConfig.experimental.mcpServer ? [
            (0, _getmcpmiddleware.getMcpMiddleware)({
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
    (0, _formaterrors.setStackFrameResolver)(async (request)=>{
        return (0, _middlewareturbopack.getOriginalStackFrames)({
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
            versionInfoCached = (0, _hotreloadersharedutils.getVersionInfo)();
        }
        return versionInfoCached;
    };
    let devtoolsFrontendUrl;
    const inspectorURLRaw = _inspector.url();
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
                const params = (0, _hotreloadersharedutils.matchNextPageBundleRequest)(req.url);
                if (params) {
                    const decodedPagePath = `/${params.path.map((param)=>decodeURIComponent(param)).join('/')}`;
                    const denormalizedPagePath = (0, _denormalizepagepath.denormalizePagePath)(decodedPagePath);
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
                                type: _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.CACHE_INDICATOR,
                                state: cacheStatus
                            });
                            cacheStatusesByHtmlRequestId.delete(htmlRequestId);
                        }
                    } else {
                        onUpgrade(client, {
                            isLegacyClient: true
                        });
                    }
                    (0, _debugchannel.connectReactDebugChannelForHtmlRequest)(htmlRequestId, sendToClient.bind(null, client));
                    (0, _serializederrors.sendSerializedErrorsToClientForHtmlRequest)(htmlRequestId, sendToClient.bind(null, client));
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
                        (0, _debugchannel.deleteReactDebugChannelForHtmlRequest)(htmlRequestId);
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
                                hotReloaderSpan.manualTraceChild(parsedData.spanName, (0, _turbopackutils.msToNs)(parsedData.startTime), (0, _turbopackutils.msToNs)(parsedData.endTime), parsedData.attributes);
                                break;
                            }
                        case 'client-hmr-latency':
                            hotReloaderSpan.manualTraceChild(parsedData.event, (0, _turbopackutils.msToNs)(parsedData.startTime), (0, _turbopackutils.msToNs)(parsedData.endTime), {
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
                                _log.warn(_messages.FAST_REFRESH_RUNTIME_RELOAD);
                            }
                            if (Array.isArray(dependencyChain) && typeof dependencyChain[0] === 'string') {
                                const cleanedModulePath = dependencyChain[0].replace(/^\[project\]/, '.').replace(/ \[.*\] \(.*\)$/, '');
                                _log.warn(`Fast Refresh had to perform a full reload when ${cleanedModulePath} changed. Read more: https://nextjs.org/docs/messages/fast-refresh-reload`);
                            }
                            break;
                        case 'client-added-page':
                            break;
                        case 'browser-logs':
                            {
                                if (nextConfig.experimental.browserDebugInfoInTerminal) {
                                    await (0, _receivelogs.receiveBrowserLogsTurbopack)({
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
                                await (0, _receivelogs.handleClientFileLogs)(parsedData.logs);
                                break;
                            }
                        case 'ping':
                            {
                                break;
                            }
                        case 'mcp-error-state-response':
                            {
                                (0, _geterrors.handleErrorStateResponse)(parsedData.requestId, parsedData.errorState, parsedData.url);
                                break;
                            }
                        case 'mcp-page-metadata-response':
                            {
                                (0, _getpagemetadata.handlePageMetadataResponse)(parsedData.requestId, parsedData.segmentTrieData, parsedData.url);
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
                    type: _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.TURBOPACK_CONNECTED,
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
                                message: (0, _utils2.formatIssue)(issue)
                            });
                        } else {
                            (0, _turbopackutils.printNonFatalIssue)(issue);
                        }
                    }
                }
                if (_devindicatorserverstate.devIndicatorServerState.disabledUntil < Date.now()) {
                    _devindicatorserverstate.devIndicatorServerState.disabledUntil = 0;
                }
                ;
                (async function() {
                    const versionInfo = await getVersionInfoCached();
                    const devToolsConfig = await (0, _devtoolsconfigmiddleware.getDevToolsConfig)(distDir);
                    const syncMessage = {
                        type: _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.SYNC,
                        errors,
                        warnings: [],
                        hash: '',
                        versionInfo,
                        debug: {
                            devtoolsFrontendUrl
                        },
                        devIndicator: _devindicatorserverstate.devIndicatorServerState,
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
                    type: _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.CACHE_INDICATOR,
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
                    (0, _debugchannel.connectReactDebugChannel)(htmlRequestId, debugChannel, sendToClient.bind(null, client));
                } else {
                    // Otherwise, we'll do that when the client connects and just store
                    // the debug channel.
                    (0, _debugchannel.setReactDebugChannelForHtmlRequest)(htmlRequestId, debugChannel);
                }
            } else if (client) {
                // The debug channel is for a subsequent request (e.g. client-side
                // navigation for server function call). If the client is not connected
                // anymore, we don't need to connect the debug channel.
                (0, _debugchannel.connectReactDebugChannel)(requestId, debugChannel, sendToClient.bind(null, client));
            }
        },
        sendErrorsToBrowser (errorsRscStream, htmlRequestId) {
            const client = clientsByHtmlRequestId.get(htmlRequestId);
            if (client) {
                // If the client is connected, we can send the errors immediately.
                (0, _serializederrors.sendSerializedErrorsToClient)(errorsRscStream, sendToClient.bind(null, client));
            } else {
                // Otherwise, store the errors stream so that we can send it when the
                // client connects.
                (0, _serializederrors.setErrorsRscStreamForHtmlRequest)(htmlRequestId, errorsRscStream);
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
            const appEntryKey = (0, _entrykey.getEntryKey)('app', 'server', page);
            const pagesEntryKey = (0, _entrykey.getEntryKey)('pages', 'server', page);
            const topLevelIssues = currentTopLevelIssues.values();
            const thisEntryIssues = currentEntryIssues.get(appEntryKey) ?? currentEntryIssues.get(pagesEntryKey);
            if (thisEntryIssues !== undefined && thisEntryIssues.size > 0) {
                // If there is an error related to the requesting page we display it instead of the first error
                return [
                    ...topLevelIssues,
                    ...thisEntryIssues.values()
                ].map((issue)=>{
                    const formattedIssue = (0, _utils2.formatIssue)(issue);
                    if (issue.severity === 'warning') {
                        (0, _turbopackutils.printNonFatalIssue)(issue);
                        return null;
                    } else if ((0, _utils2.isWellKnownError)(issue)) {
                        _log.error(formattedIssue);
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
                    errors.push(Object.defineProperty(new Error((0, _utils2.formatIssue)(issue)), "__NEXT_ERROR_CODE", {
                        value: "E394",
                        enumerable: false,
                        configurable: true
                    }));
                }
            }
            for (const entryIssues of currentEntryIssues.values()){
                for (const issue of entryIssues.values()){
                    if (issue.severity !== 'warning') {
                        const message = (0, _utils2.formatIssue)(issue);
                        errors.push(Object.defineProperty(new Error(message), "__NEXT_ERROR_CODE", {
                            value: "E394",
                            enumerable: false,
                            configurable: true
                        }));
                    } else {
                        (0, _turbopackutils.printNonFatalIssue)(issue);
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
                await (0, _renderserver.clearAllModuleContexts)();
                this.send({
                    type: _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.SERVER_COMPONENT_CHANGES,
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
                if (_constants.BLOCKED_PAGES.includes(inputPage) && inputPage !== '/_error') {
                    return;
                }
                await currentEntriesHandling;
                // TODO We shouldn't look into the filesystem again. This should use the information from entrypoints
                let routeDef = definition ?? await (0, _ondemandentryhandler.findPagePathData)(projectPath, inputPage, nextConfig.pageExtensions, opts.pagesDir, opts.appDir, !!nextConfig.experimental.globalNotFound);
                // If the route is actually an app page route, then we should have access
                // to the app route definition, and therefore, the appPaths from it.
                if (!appPaths && definition && (0, _apppageroutedefinition.isAppPageRouteDefinition)(definition)) {
                    appPaths = definition.appPaths;
                }
                let page = routeDef.page;
                if (appPaths) {
                    const normalizedPage = (0, _apppaths.normalizeAppPath)(page);
                    // filter out paths that are not exact matches (e.g. catchall)
                    const matchingAppPaths = appPaths.filter((path)=>(0, _apppaths.normalizeAppPath)(path) === normalizedPage);
                    // the last item in the array is the root page, if there are parallel routes
                    page = matchingAppPaths[matchingAppPaths.length - 1];
                }
                const pathname = (definition == null ? void 0 : definition.pathname) ?? inputPage;
                if (page === '/_error') {
                    let finishBuilding = startBuilding(pathname, requestUrl, false);
                    try {
                        await (0, _turbopackutils.handlePagesErrorRoute)({
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
                const isEntryMetadataRouteFile = (0, _ismetadataroute.isMetadataRouteFile)(routeDef.filename.replace(opts.appDir || '', ''), nextConfig.pageExtensions, true);
                const normalizedAppPage = isEntryMetadataRouteFile ? (0, _turbopackutils.normalizedPageToTurbopackStructureRoute)(page, (0, _path.extname)(routeDef.filename)) : page;
                const route = isInsideAppDir ? currentEntrypoints.app.get(normalizedAppPage) : currentEntrypoints.page.get(page);
                if (!route) {
                    // TODO: why is this entry missing in turbopack?
                    if (page === '/middleware') return;
                    if (page === '/src/middleware') return;
                    if (page === '/proxy') return;
                    if (page === '/src/proxy') return;
                    if (page === '/instrumentation') return;
                    if (page === '/src/instrumentation') return;
                    throw new _utils.PageNotFoundError(`route not found ${page}`);
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
                    await (0, _turbopackutils.handleRouteType)({
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
            (0, _mcptelemetrytracker.recordMcpTelemetry)(opts.telemetry);
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
                            type: _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.BUILDING
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
                                const message = (0, _utils2.formatIssue)(issue);
                                errorsMap.set(key, {
                                    message,
                                    details: issue.detail ? (0, _utils2.renderStyledStringToErrorAnsi)(issue.detail) : undefined
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
                                type: _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.BUILT,
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
                            _log.event(`Compiled in ${timeMessage}`);
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