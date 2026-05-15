"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
0 && (module.exports = {
    turbopackBuild: null,
    waitForShutdown: null,
    workerMain: null
});
function _export(target, all) {
    for(var name in all)Object.defineProperty(target, name, {
        enumerable: true,
        get: all[name]
    });
}
_export(exports, {
    turbopackBuild: function() {
        return turbopackBuild;
    },
    waitForShutdown: function() {
        return waitForShutdown;
    },
    workerMain: function() {
        return workerMain;
    }
});
const _path = /*#__PURE__*/ _interop_require_default(require("path"));
const _turbopackwarning = require("../../lib/turbopack-warning");
const _utils = require("../../shared/lib/turbopack/utils");
const _buildcontext = require("../build-context");
const _swc = require("../swc");
const _installbindings = require("../swc/install-bindings");
const _handleentrypoints = require("../handle-entrypoints");
const _manifestloader = require("../../shared/lib/turbopack/manifest-loader");
const _fs = require("fs");
const _constants = require("../../shared/lib/constants");
const _config = /*#__PURE__*/ _interop_require_default(require("../../server/config"));
const _utils1 = require("../../export/utils");
const _storage = require("../../telemetry/storage");
const _trace = require("../../trace");
const _ciinfo = require("../../server/ci-info");
const _compilationevents = require("../../shared/lib/turbopack/compilation-events");
const _utils2 = require("../utils");
const _normalizepath = require("../../lib/normalize-path");
function _interop_require_default(obj) {
    return obj && obj.__esModule ? obj : {
        default: obj
    };
}
async function turbopackBuild() {
    var _config_turbopack, _config_turbopack1, _config_experimental;
    await (0, _turbopackwarning.validateTurboNextConfig)({
        dir: _buildcontext.NextBuildContext.dir,
        configPhase: _constants.PHASE_PRODUCTION_BUILD
    });
    const config = _buildcontext.NextBuildContext.config;
    const dir = _buildcontext.NextBuildContext.dir;
    const distDir = _buildcontext.NextBuildContext.distDir;
    const buildId = _buildcontext.NextBuildContext.buildId;
    const encryptionKey = _buildcontext.NextBuildContext.encryptionKey;
    const previewProps = _buildcontext.NextBuildContext.previewProps;
    const hasRewrites = _buildcontext.NextBuildContext.hasRewrites;
    const rewrites = _buildcontext.NextBuildContext.rewrites;
    const noMangling = _buildcontext.NextBuildContext.noMangling;
    const currentNodeJsVersion = process.versions.node;
    const startTime = process.hrtime();
    const bindings = (0, _swc.getBindingsSync)() // our caller should have already loaded these
    ;
    const dev = false;
    const supportedBrowsers = (0, _utils2.getSupportedBrowsers)(dir, dev);
    const persistentCaching = (0, _utils.isFileSystemCacheEnabledForBuild)(config);
    const rootPath = ((_config_turbopack = config.turbopack) == null ? void 0 : _config_turbopack.root) || config.outputFileTracingRoot || dir;
    const project = await bindings.turbo.createProject({
        rootPath: ((_config_turbopack1 = config.turbopack) == null ? void 0 : _config_turbopack1.root) || config.outputFileTracingRoot || dir,
        projectPath: (0, _normalizepath.normalizePath)(_path.default.relative(rootPath, dir) || '.'),
        distDir,
        nextConfig: config,
        watch: {
            enable: false
        },
        dev,
        env: process.env,
        defineEnv: (0, _swc.createDefineEnv)({
            isTurbopack: true,
            clientRouterFilters: _buildcontext.NextBuildContext.clientRouterFilters,
            config,
            dev,
            distDir,
            projectPath: dir,
            fetchCacheKeyPrefix: config.experimental.fetchCacheKeyPrefix,
            hasRewrites,
            // Implemented separately in Turbopack, doesn't have to be passed here.
            middlewareMatchers: undefined,
            rewrites
        }),
        buildId,
        encryptionKey,
        previewProps,
        browserslistQuery: supportedBrowsers.join(', '),
        noMangling,
        writeRoutesHashesManifest: !!process.env.NEXT_TURBOPACK_WRITE_ROUTES_HASHES_MANIFEST,
        currentNodeJsVersion
    }, {
        persistentCaching,
        memoryLimit: (_config_experimental = config.experimental) == null ? void 0 : _config_experimental.turbopackMemoryLimit,
        dependencyTracking: persistentCaching,
        isCi: _ciinfo.isCI,
        isShortSession: true
    });
    try {
        (0, _compilationevents.backgroundLogCompilationEvents)(project);
        // Write an empty file in a known location to signal this was built with Turbopack
        await _fs.promises.writeFile(_path.default.join(distDir, 'turbopack'), '');
        await _fs.promises.mkdir(_path.default.join(distDir, 'server'), {
            recursive: true
        });
        await _fs.promises.mkdir(_path.default.join(distDir, 'static', buildId), {
            recursive: true
        });
        await _fs.promises.writeFile(_path.default.join(distDir, 'package.json'), '{"type": "commonjs"}');
        let appDirOnly = _buildcontext.NextBuildContext.appDirOnly;
        const entrypoints = await project.writeAllEntrypointsToDisk(appDirOnly);
        (0, _utils2.printBuildErrors)(entrypoints, dev);
        let routes = entrypoints.routes;
        if (!routes) {
            // This should never ever happen, there should be an error issue, or the bindings call should
            // have thrown.
            throw Object.defineProperty(new Error(`Turbopack build failed`), "__NEXT_ERROR_CODE", {
                value: "E853",
                enumerable: false,
                configurable: true
            });
        }
        const hasPagesEntries = Array.from(routes.values()).some((route)=>{
            if (route.type === 'page' || route.type === 'page-api') {
                return true;
            }
            return false;
        });
        // If there's no pages entries, then we are in app-dir-only mode
        if (!hasPagesEntries) {
            appDirOnly = true;
        }
        const manifestLoader = new _manifestloader.TurbopackManifestLoader({
            buildId,
            distDir,
            encryptionKey
        });
        const currentEntrypoints = await (0, _handleentrypoints.rawEntrypointsToEntrypoints)(entrypoints);
        const promises = [];
        if (!appDirOnly) {
            for (const [page, route] of currentEntrypoints.page){
                promises.push((0, _handleentrypoints.handleRouteType)({
                    page,
                    route,
                    manifestLoader
                }));
            }
        }
        for (const [page, route] of currentEntrypoints.app){
            promises.push((0, _handleentrypoints.handleRouteType)({
                page,
                route,
                manifestLoader
            }));
        }
        await Promise.all(promises);
        await Promise.all([
            // Only load pages router manifests if not app-only
            ...!appDirOnly ? [
                manifestLoader.loadBuildManifest('_app'),
                manifestLoader.loadPagesManifest('_app'),
                manifestLoader.loadFontManifest('_app'),
                manifestLoader.loadPagesManifest('_document'),
                manifestLoader.loadClientBuildManifest('_error'),
                manifestLoader.loadBuildManifest('_error'),
                manifestLoader.loadPagesManifest('_error'),
                manifestLoader.loadFontManifest('_error')
            ] : [],
            entrypoints.instrumentation && manifestLoader.loadMiddlewareManifest('instrumentation', 'instrumentation'),
            entrypoints.middleware && await manifestLoader.loadMiddlewareManifest('middleware', 'middleware')
        ]);
        manifestLoader.writeManifests({
            devRewrites: undefined,
            productionRewrites: rewrites,
            entrypoints: currentEntrypoints
        });
        if (_buildcontext.NextBuildContext.analyze) {
            await project.writeAnalyzeData(appDirOnly);
        }
        const shutdownPromise = project.shutdown();
        const time = process.hrtime(startTime);
        return {
            duration: time[0] + time[1] / 1e9,
            buildTraceContext: undefined,
            shutdownPromise
        };
    } catch (err) {
        await project.shutdown();
        throw err;
    }
}
let shutdownPromise;
async function workerMain(workerData) {
    var _config_experimental;
    // setup new build context from the serialized data passed from the parent
    Object.assign(_buildcontext.NextBuildContext, workerData.buildContext);
    /// load the config because it's not serializable
    const config = _buildcontext.NextBuildContext.config = await (0, _config.default)(_constants.PHASE_PRODUCTION_BUILD, _buildcontext.NextBuildContext.dir, {
        debugPrerender: _buildcontext.NextBuildContext.debugPrerender,
        reactProductionProfiling: _buildcontext.NextBuildContext.reactProductionProfiling
    });
    // Matches handling in build/index.ts
    // https://github.com/vercel/next.js/blob/84f347fc86f4efc4ec9f13615c215e4b9fb6f8f0/packages/next/src/build/index.ts#L815-L818
    // Ensures the `config.distDir` option is matched.
    if ((0, _utils1.hasCustomExportOutput)(_buildcontext.NextBuildContext.config)) {
        _buildcontext.NextBuildContext.config.distDir = '.next';
    }
    // Clone the telemetry for worker
    const telemetry = new _storage.Telemetry({
        distDir: _buildcontext.NextBuildContext.config.distDir
    });
    (0, _trace.setGlobal)('telemetry', telemetry);
    // Install bindings early so we can access synchronously later
    await (0, _installbindings.installBindings)((_config_experimental = config.experimental) == null ? void 0 : _config_experimental.useWasmBinary);
    try {
        const { shutdownPromise: resultShutdownPromise, buildTraceContext, duration } = await turbopackBuild();
        shutdownPromise = resultShutdownPromise;
        return {
            buildTraceContext,
            duration
        };
    } finally{
        // Always flush telemetry before worker exits (waits for async operations like setTimeout in debug mode)
        await telemetry.flush();
    }
}
async function waitForShutdown() {
    if (shutdownPromise) {
        await shutdownPromise;
    }
}

//# sourceMappingURL=impl.js.map