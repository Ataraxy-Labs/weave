"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "default", {
    enumerable: true,
    get: function() {
        return analyze;
    }
});
const _trace = require("../../trace");
const _log = /*#__PURE__*/ _interop_require_wildcard(require("../output/log"));
const _nodepath = /*#__PURE__*/ _interop_require_wildcard(require("node:path"));
const _config = /*#__PURE__*/ _interop_require_default(require("../../server/config"));
const _constants = require("../../shared/lib/constants");
const _turbopackanalyze = require("../turbopack-analyze");
const _durationtostring = require("../duration-to-string");
const _promises = require("node:fs/promises");
const _entries = require("../entries");
const _findpagefile = require("../../server/lib/find-page-file");
const _findpagesdir = require("../../lib/find-pages-dir");
const _pagetypes = require("../../lib/page-types");
const _loadcustomroutes = /*#__PURE__*/ _interop_require_default(require("../../lib/load-custom-routes"));
const _generateroutesmanifest = require("../generate-routes-manifest");
const _ppr = require("../../server/lib/experimental/ppr");
const _apppaths = require("../../shared/lib/router/utils/app-paths");
const _nodehttp = /*#__PURE__*/ _interop_require_default(require("node:http"));
const _servehandler = /*#__PURE__*/ _interop_require_default(require("next/dist/compiled/serve-handler"));
const _storage = require("../../telemetry/storage");
const _events = require("../../telemetry/events");
const _shared = require("../../trace/shared");
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
async function analyze({ dir, reactProductionProfiling = false, noMangling = false, appDirOnly = false, output = false, port = 4000 }) {
    try {
        const config = await (0, _config.default)(_constants.PHASE_ANALYZE, dir, {
            silent: false,
            reactProductionProfiling
        });
        process.env.NEXT_DEPLOYMENT_ID = config.deploymentId || '';
        const distDir = _nodepath.join(dir, '.next');
        const telemetry = new _storage.Telemetry({
            distDir
        });
        (0, _trace.setGlobal)('phase', _constants.PHASE_ANALYZE);
        (0, _trace.setGlobal)('distDir', distDir);
        (0, _trace.setGlobal)('telemetry', telemetry);
        _log.info('Analyzing a production build...');
        const analyzeContext = {
            config,
            dir,
            distDir,
            noMangling,
            appDirOnly
        };
        const { duration: analyzeDuration, shutdownPromise } = await (0, _turbopackanalyze.turbopackAnalyze)(analyzeContext);
        const durationString = (0, _durationtostring.durationToString)(analyzeDuration);
        const analyzeDir = _nodepath.join(distDir, 'diagnostics/analyze');
        await shutdownPromise;
        const routes = await collectRoutesForAnalyze(dir, config, appDirOnly);
        await (0, _promises.cp)(_nodepath.join(__dirname, '../../bundle-analyzer'), analyzeDir, {
            recursive: true
        });
        await (0, _promises.mkdir)(_nodepath.join(analyzeDir, 'data'), {
            recursive: true
        });
        await (0, _promises.writeFile)(_nodepath.join(analyzeDir, 'data', 'routes.json'), JSON.stringify(routes, null, 2));
        let logMessage = `Analyze completed in ${durationString}.`;
        if (output) {
            logMessage += ` Results written to ${analyzeDir}.\nTo explore the analyze results interactively, run \`next experimental-analyze\` without \`--output\`.`;
        }
        _log.event(logMessage);
        telemetry.record((0, _events.eventAnalyzeCompleted)({
            success: true,
            durationInSeconds: Math.round(analyzeDuration),
            totalPageCount: routes.length
        }));
        if (!output) {
            await startServer(analyzeDir, port);
        }
    } catch (e) {
        const telemetry = _shared.traceGlobals.get('telemetry');
        if (telemetry) {
            telemetry.record((0, _events.eventAnalyzeCompleted)({
                success: false
            }));
        }
        throw e;
    }
}
/**
 * Collects all routes from the project for the bundle analyzer.
 * Returns a list of route paths (both static and dynamic).
 */ async function collectRoutesForAnalyze(dir, config, appDirOnly) {
    const { pagesDir, appDir } = (0, _findpagesdir.findPagesDir)(dir);
    const validFileMatcher = (0, _findpagefile.createValidFileMatcher)(config.pageExtensions, appDir);
    let appType;
    if (pagesDir && appDir) {
        appType = 'hybrid';
    } else if (pagesDir) {
        appType = 'pages';
    } else if (appDir) {
        appType = 'app';
    } else {
        throw Object.defineProperty(new Error('No pages or app directory found.'), "__NEXT_ERROR_CODE", {
            value: "E929",
            enumerable: false,
            configurable: true
        });
    }
    const { appPaths } = appDir ? await (0, _entries.collectAppFiles)(appDir, validFileMatcher) : {
        appPaths: []
    };
    const pagesPaths = pagesDir ? await (0, _entries.collectPagesFiles)(pagesDir, validFileMatcher) : null;
    const appMapping = await (0, _entries.createPagesMapping)({
        pagePaths: appPaths,
        isDev: false,
        pagesType: _pagetypes.PAGE_TYPES.APP,
        pageExtensions: config.pageExtensions,
        pagesDir,
        appDir,
        appDirOnly
    });
    const pagesMapping = pagesPaths ? await (0, _entries.createPagesMapping)({
        pagePaths: pagesPaths,
        isDev: false,
        pagesType: _pagetypes.PAGE_TYPES.PAGES,
        pageExtensions: config.pageExtensions,
        pagesDir,
        appDir,
        appDirOnly
    }) : null;
    const pageKeys = {
        pages: pagesMapping ? Object.keys(pagesMapping) : [],
        app: appMapping ? Object.keys(appMapping).map((key)=>(0, _apppaths.normalizeAppPath)(key)) : undefined
    };
    // Load custom routes
    const { redirects, headers, rewrites } = await (0, _loadcustomroutes.default)(config);
    // Compute restricted redirect paths
    const restrictedRedirectPaths = [
        '/_next'
    ].map((pathPrefix)=>config.basePath ? `${config.basePath}${pathPrefix}` : pathPrefix);
    const isAppPPREnabled = (0, _ppr.checkIsAppPPREnabled)(config.experimental.ppr);
    // Generate routes manifest
    const { routesManifest } = (0, _generateroutesmanifest.generateRoutesManifest)({
        appType,
        pageKeys,
        config,
        redirects,
        headers,
        rewrites,
        restrictedRedirectPaths,
        isAppPPREnabled
    });
    return routesManifest.dynamicRoutes.map((r)=>r.page).concat(routesManifest.staticRoutes.map((r)=>r.page));
}
function startServer(dir, port) {
    const server = _nodehttp.default.createServer((req, res)=>{
        return (0, _servehandler.default)(req, res, {
            public: dir
        });
    });
    return new Promise((resolve, reject)=>{
        function onError(err) {
            server.close(()=>{
                reject(err);
            });
        }
        server.on('error', onError);
        server.listen(port, 'localhost', ()=>{
            const address = server.address();
            if (address == null) {
                reject(Object.defineProperty(new Error('Unable to get server address'), "__NEXT_ERROR_CODE", {
                    value: "E928",
                    enumerable: false,
                    configurable: true
                }));
                return;
            }
            // No longer needed after startup
            server.removeListener('error', onError);
            let addressString;
            if (typeof address === 'string') {
                addressString = address;
            } else if (address.family === 'IPv6' && (address.address === '::' || address.address === '::1')) {
                addressString = `localhost:${address.port}`;
            } else if (address.family === 'IPv6') {
                addressString = `[${address.address}]:${address.port}`;
            } else {
                addressString = `${address.address}:${address.port}`;
            }
            _log.info(`Bundle analyzer available at http://${addressString}`);
            resolve();
        });
    });
}

//# sourceMappingURL=index.js.map