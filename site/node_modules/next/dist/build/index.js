"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
0 && (module.exports = {
    createStaticWorker: null,
    default: null
});
function _export(target, all) {
    for(var name in all)Object.defineProperty(target, name, {
        enumerable: true,
        get: all[name]
    });
}
_export(exports, {
    createStaticWorker: function() {
        return createStaticWorker;
    },
    default: function() {
        return build;
    }
});
require("../lib/setup-exception-listeners");
const _env = require("@next/env");
const _picocolors = require("../lib/picocolors");
const _picomatch = require("next/dist/compiled/picomatch");
const _fs = require("fs");
const _os = /*#__PURE__*/ _interop_require_default(require("os"));
const _worker = require("../lib/worker");
const _configshared = require("../server/config-shared");
const _devalue = /*#__PURE__*/ _interop_require_default(require("next/dist/compiled/devalue"));
const _findup = /*#__PURE__*/ _interop_require_default(require("next/dist/compiled/find-up"));
const _indexcjs = require("next/dist/compiled/nanoid/index.cjs");
const _path = /*#__PURE__*/ _interop_require_default(require("path"));
const _constants = require("../lib/constants");
const _fileexists = require("../lib/file-exists");
const _findpagesdir = require("../lib/find-pages-dir");
const _loadcustomroutes = /*#__PURE__*/ _interop_require_wildcard(require("../lib/load-custom-routes"));
const _nonnullable = require("../lib/non-nullable");
const _recursivedelete = require("../lib/recursive-delete");
const _verifypartytownsetup = require("../lib/verify-partytown-setup");
const _constants1 = require("../shared/lib/constants");
const _entryconstants = require("../shared/lib/entry-constants");
const _utils = require("../shared/lib/router/utils");
const _bundler = require("../lib/bundler");
const _config = /*#__PURE__*/ _interop_require_default(require("../server/config"));
const _normalizepagepath = require("../shared/lib/page-path/normalize-page-path");
const _require = require("../server/require");
const _ciinfo = /*#__PURE__*/ _interop_require_wildcard(require("../server/ci-info"));
const _turborepoaccesstrace = require("./turborepo-access-trace");
const _events = require("../telemetry/events");
const _storage = require("../telemetry/storage");
const _entries = require("./entries");
const _sortbypageexts = require("./sort-by-page-exts");
const _getstaticinfoincludinglayouts = require("./get-static-info-including-layouts");
const _pagetypes = require("../lib/page-types");
const _generatebuildid = require("./generate-build-id");
const _iswriteable = require("./is-writeable");
const _log = /*#__PURE__*/ _interop_require_wildcard(require("./output/log"));
const _spinner = /*#__PURE__*/ _interop_require_default(require("./spinner"));
const _trace = require("../trace");
const _utils1 = require("./utils");
const _writebuildid = require("./write-build-id");
const _normalizelocalepath = require("../shared/lib/i18n/normalize-locale-path");
const _iserror = /*#__PURE__*/ _interop_require_default(require("../lib/is-error"));
const _isedgeruntime = require("../lib/is-edge-runtime");
const _recursivecopy = require("../lib/recursive-copy");
const _swc = require("./swc");
const _installbindings = require("./swc/install-bindings");
const _routeregex = require("../shared/lib/router/utils/route-regex");
const _getfilesindir = require("../lib/get-files-in-dir");
const _swcplugins = require("../telemetry/events/swc-plugins");
const _apppaths = require("../shared/lib/router/utils/app-paths");
const _approuterheaders = require("../client/components/app-router-headers");
const _webpackbuild = require("./webpack-build");
const _buildcontext = require("./build-context");
const _normalizepathsep = require("../shared/lib/page-path/normalize-path-sep");
const _isapprouteroute = require("../lib/is-app-route-route");
const _createclientrouterfilter = require("../lib/create-client-router-filter");
const _findpagefile = require("../server/lib/find-page-file");
const _typecheck = require("./type-check");
const _generateinterceptionroutesrewrites = require("../lib/generate-interception-routes-rewrites");
const _builddataroute = require("../server/lib/router-utils/build-data-route");
const _collectbuildtraces = require("./collect-build-traces");
const _formatmanifest = require("./manifests/formatter/format-manifest");
const _builddiagnostics = require("../diagnostics/build-diagnostics");
const _appinfolog = require("../server/lib/app-info-log");
const _utils2 = require("../export/utils");
const _trace1 = require("../lib/memory/trace");
const _encryptionutilsserver = require("../server/app-render/encryption-utils-server");
const _uploadtrace = /*#__PURE__*/ _interop_require_default(require("../trace/upload-trace"));
const _ppr = require("../server/lib/experimental/ppr");
const _fallback = require("../lib/fallback");
const _renderingmode = require("./rendering-mode");
const _invarianterror = require("../shared/lib/invariant-error");
const _isbot = require("../shared/lib/router/utils/is-bot");
const _turbopackbuild = require("./turbopack-build");
const _utils3 = require("../shared/lib/turbopack/utils");
const _inlinestaticenv = require("../lib/inline-static-env");
const _staticenv = require("../lib/static-env");
const _durationtostring = require("./duration-to-string");
const _shared = require("../trace/shared");
const _errortelemetryutils = require("../lib/error-telemetry-utils");
const _afterproductioncompile = require("./after-production-compile");
const _previewkeyutils = require("./preview-key-utils");
const _buildcomplete = require("./adapter/build-complete");
const _sortableroutes = require("../shared/lib/router/utils/sortable-routes");
const _promises = require("fs/promises");
const _routetypesutils = require("../server/lib/router-utils/route-types-utils");
const _lockfile = require("./lockfile");
const _buildprefetchsegmentdataroute = require("../server/lib/router-utils/build-prefetch-segment-data-route");
const _generateroutesmanifest = require("./generate-routes-manifest");
const _validateapppaths = require("./validate-app-paths");
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
/**
 * The headers that are allowed to be used when revalidating routes. Currently
 * this includes both headers used by the pages and app routers.
 */ const ALLOWED_HEADERS = [
    'host',
    _constants.MATCHED_PATH_HEADER,
    _constants.PRERENDER_REVALIDATE_HEADER,
    _constants.PRERENDER_REVALIDATE_ONLY_GENERATED_HEADER,
    _constants.NEXT_CACHE_REVALIDATED_TAGS_HEADER,
    _constants.NEXT_CACHE_REVALIDATE_TAG_TOKEN_HEADER
];
function getCacheDir(distDir) {
    const cacheDir = _path.default.join(distDir, 'cache');
    if (_ciinfo.isCI && !_ciinfo.hasNextSupport) {
        const hasCache = (0, _fs.existsSync)(cacheDir);
        if (!hasCache) {
            // Intentionally not piping to stderr which is what `Log.warn` does in case people fail in CI when
            // stderr is detected.
            console.log(`${_log.prefixes.warn} No build cache found. Please configure build caching for faster rebuilds. Read more: https://nextjs.org/docs/messages/no-cache`);
        }
    }
    return cacheDir;
}
async function writeFileUtf8(filePath, content) {
    await _fs.promises.writeFile(filePath, content, 'utf-8');
}
function readFileUtf8(filePath) {
    return _fs.promises.readFile(filePath, 'utf8');
}
async function writeManifest(filePath, manifest) {
    await writeFileUtf8(filePath, (0, _formatmanifest.formatManifest)(manifest));
}
async function readManifest(filePath) {
    return JSON.parse(await readFileUtf8(filePath));
}
async function writePrerenderManifest(distDir, manifest) {
    await writeManifest(_path.default.join(distDir, _constants1.PRERENDER_MANIFEST), manifest);
}
async function writeClientSsgManifest(prerenderManifest, { buildId, distDir, locales }) {
    const ssgPages = new Set([
        ...Object.entries(prerenderManifest.routes)// Filter out dynamic routes
        .filter(([, { srcRoute }])=>srcRoute == null).map(([route])=>(0, _normalizelocalepath.normalizeLocalePath)(route, locales).pathname),
        ...Object.keys(prerenderManifest.dynamicRoutes)
    ].sort());
    const clientSsgManifestContent = `self.__SSG_MANIFEST=${(0, _devalue.default)(ssgPages)};self.__SSG_MANIFEST_CB&&self.__SSG_MANIFEST_CB()`;
    await writeFileUtf8(_path.default.join(distDir, _constants1.CLIENT_STATIC_FILES_PATH, buildId, '_ssgManifest.js'), clientSsgManifestContent);
}
async function writeFunctionsConfigManifest(distDir, manifest) {
    let sortedManifest = {
        version: manifest.version,
        functions: Object.fromEntries(Object.entries(manifest.functions).sort(([a], [b])=>a.localeCompare(b)))
    };
    await writeManifest(_path.default.join(distDir, _constants1.SERVER_DIRECTORY, _constants1.FUNCTIONS_CONFIG_MANIFEST), sortedManifest);
}
async function writeRequiredServerFilesManifest(distDir, requiredServerFiles) {
    await writeManifest(_path.default.join(distDir, _constants1.SERVER_FILES_MANIFEST + '.json'), requiredServerFiles);
    await writeFileUtf8(_path.default.join(distDir, _constants1.SERVER_FILES_MANIFEST + '.js'), `self.__SERVER_FILES_MANIFEST=${(0, _formatmanifest.formatManifest)(requiredServerFiles)}`);
}
async function writeImagesManifest(distDir, config) {
    var _config_images, _config_images1;
    const images = {
        ...config.images
    };
    const { deviceSizes, imageSizes } = images;
    images.sizes = [
        ...deviceSizes,
        ...imageSizes
    ];
    // By default, remotePatterns will allow no remote images ([])
    images.remotePatterns = ((config == null ? void 0 : (_config_images = config.images) == null ? void 0 : _config_images.remotePatterns) || []).map((p)=>{
        var _p_protocol;
        return {
            // Modifying the manifest should also modify matchRemotePattern()
            protocol: (_p_protocol = p.protocol) == null ? void 0 : _p_protocol.replace(/:$/, ''),
            hostname: (0, _picomatch.makeRe)(p.hostname).source,
            port: p.port,
            pathname: (0, _picomatch.makeRe)(p.pathname ?? '**', {
                dot: true
            }).source,
            search: p.search
        };
    });
    // By default, localPatterns will allow all local images (undefined)
    if (config == null ? void 0 : (_config_images1 = config.images) == null ? void 0 : _config_images1.localPatterns) {
        images.localPatterns = config.images.localPatterns.map((p)=>({
                // Modifying the manifest should also modify matchLocalPattern()
                pathname: (0, _picomatch.makeRe)(p.pathname ?? '**', {
                    dot: true
                }).source,
                search: p.search
            }));
    }
    await writeManifest(_path.default.join(distDir, _constants1.IMAGES_MANIFEST), {
        version: 1,
        images
    });
}
const STANDALONE_DIRECTORY = 'standalone';
async function writeStandaloneDirectory(nextBuildSpan, distDir, pageKeys, denormalizedAppPages, outputFileTracingRoot, requiredServerFiles, middlewareManifest, hasNodeMiddleware, hasInstrumentationHook, staticPages, loadedEnvFiles, appDir) {
    await nextBuildSpan.traceChild('write-standalone-directory').traceAsyncFn(async ()=>{
        await (0, _utils1.copyTracedFiles)(// requiredServerFiles.appDir Refers to the application directory, not App Router.
        requiredServerFiles.appDir, distDir, pageKeys.pages, denormalizedAppPages, outputFileTracingRoot, requiredServerFiles.config, middlewareManifest, hasNodeMiddleware, hasInstrumentationHook, staticPages);
        for (const file of [
            ...requiredServerFiles.files,
            _path.default.join(requiredServerFiles.config.distDir, _constants1.SERVER_FILES_MANIFEST + '.json'),
            ...loadedEnvFiles.reduce((acc, envFile)=>{
                if ([
                    '.env',
                    '.env.production'
                ].includes(envFile.path)) {
                    acc.push(envFile.path);
                }
                return acc;
            }, [])
        ]){
            // requiredServerFiles.appDir Refers to the application directory, not App Router.
            const filePath = _path.default.join(requiredServerFiles.appDir, file);
            const outputPath = _path.default.join(distDir, STANDALONE_DIRECTORY, _path.default.relative(outputFileTracingRoot, filePath));
            await _fs.promises.mkdir(_path.default.dirname(outputPath), {
                recursive: true
            });
            await _fs.promises.copyFile(filePath, outputPath);
        }
        if (hasNodeMiddleware) {
            const middlewareOutput = _path.default.join(distDir, STANDALONE_DIRECTORY, _path.default.relative(outputFileTracingRoot, distDir), _constants1.SERVER_DIRECTORY, 'middleware.js');
            await _fs.promises.mkdir(_path.default.dirname(middlewareOutput), {
                recursive: true
            });
            await _fs.promises.copyFile(_path.default.join(distDir, _constants1.SERVER_DIRECTORY, 'middleware.js'), middlewareOutput);
        }
        const originalPagesDir = _path.default.join(distDir, _constants1.SERVER_DIRECTORY, 'pages');
        if ((0, _fs.existsSync)(originalPagesDir)) {
            await (0, _recursivecopy.recursiveCopy)(originalPagesDir, _path.default.join(distDir, STANDALONE_DIRECTORY, _path.default.relative(outputFileTracingRoot, distDir), _constants1.SERVER_DIRECTORY, 'pages'), {
                overwrite: true
            });
        }
        if (appDir) {
            const originalServerApp = _path.default.join(distDir, _constants1.SERVER_DIRECTORY, 'app');
            if ((0, _fs.existsSync)(originalServerApp)) {
                await (0, _recursivecopy.recursiveCopy)(originalServerApp, _path.default.join(distDir, STANDALONE_DIRECTORY, _path.default.relative(outputFileTracingRoot, distDir), _constants1.SERVER_DIRECTORY, 'app'), {
                    overwrite: true
                });
            }
        }
    });
}
function getNumberOfWorkers(config) {
    if (config.experimental.cpus && config.experimental.cpus !== _configshared.defaultConfig.experimental.cpus) {
        return config.experimental.cpus;
    }
    if (config.experimental.memoryBasedWorkersCount) {
        return Math.max(Math.min(config.experimental.cpus || 1, Math.floor(_os.default.freemem() / 1e9)), // enforce a minimum of 4 workers
        4);
    }
    if (config.experimental.cpus) {
        return config.experimental.cpus;
    }
    // Fall back to 4 workers if a count is not specified
    return 4;
}
const staticWorkerPath = require.resolve('./worker');
const staticWorkerExposedMethods = [
    'hasCustomGetInitialProps',
    'isPageStatic',
    'getDefinedNamedExports',
    'exportPages'
];
function createStaticWorker(config, options) {
    const { numberOfWorkers, debuggerPortOffset, progress } = options;
    return new _worker.Worker(staticWorkerPath, {
        logger: _log,
        numWorkers: numberOfWorkers,
        onActivity: ()=>{
            progress == null ? void 0 : progress.run();
        },
        onActivityAbort: ()=>{
            progress == null ? void 0 : progress.clear();
        },
        debuggerPortOffset,
        enableSourceMaps: config.enablePrerenderSourceMaps,
        // remove --max-old-space-size flag as it can cause memory issues.
        isolatedMemory: true,
        enableWorkerThreads: config.experimental.workerThreads,
        exposedMethods: staticWorkerExposedMethods
    });
}
async function writeFullyStaticExport(config, dir, enabledDirectories, configOutDir, nextBuildSpan, appDirOnly) {
    const exportApp = require('../export').default;
    await exportApp(dir, {
        buildExport: false,
        nextConfig: config,
        enabledDirectories,
        silent: true,
        outdir: _path.default.join(dir, configOutDir),
        numWorkers: getNumberOfWorkers(config),
        appDirOnly
    }, nextBuildSpan);
}
async function getBuildId(isGenerateMode, distDir, nextBuildSpan, config) {
    if (isGenerateMode) {
        return await _fs.promises.readFile(_path.default.join(distDir, 'BUILD_ID'), 'utf8');
    }
    return await nextBuildSpan.traceChild('generate-buildid').traceAsyncFn(()=>(0, _generatebuildid.generateBuildId)(config.generateBuildId, _indexcjs.nanoid));
}
async function build(dir, experimentalAnalyze = false, reactProductionProfiling = false, debugOutput = false, debugPrerender = false, noMangling = false, appDirOnly = false, bundler = _bundler.Bundler.Turbopack, experimentalBuildMode, traceUploadUrl, debugBuildAppPaths, debugBuildPagePaths) {
    const isCompileMode = experimentalBuildMode === 'compile';
    const isGenerateMode = experimentalBuildMode === 'generate';
    _buildcontext.NextBuildContext.isCompileMode = isCompileMode;
    _buildcontext.NextBuildContext.analyze = experimentalAnalyze;
    const buildStartTime = Date.now();
    let appType;
    let loadedConfig;
    let staticWorker;
    try {
        const nextBuildSpan = (0, _trace.trace)('next-build', undefined, {
            buildMode: experimentalBuildMode,
            version: "16.1.6"
        });
        _buildcontext.NextBuildContext.nextBuildSpan = nextBuildSpan;
        _buildcontext.NextBuildContext.dir = dir;
        _buildcontext.NextBuildContext.appDirOnly = appDirOnly;
        _buildcontext.NextBuildContext.reactProductionProfiling = reactProductionProfiling;
        _buildcontext.NextBuildContext.noMangling = noMangling;
        _buildcontext.NextBuildContext.debugPrerender = debugPrerender;
        await nextBuildSpan.traceAsyncFn(async ()=>{
            var _config_experimental, _mappedPages_404, _mappedPages__error;
            // attempt to load global env values so they are available in next.config.js
            const { loadedEnvFiles } = nextBuildSpan.traceChild('load-dotenv').traceFn(()=>(0, _env.loadEnvConfig)(dir, false, _log));
            _buildcontext.NextBuildContext.loadedEnvFiles = loadedEnvFiles;
            const turborepoAccessTraceResult = new _turborepoaccesstrace.TurborepoAccessTraceResult();
            const config = await nextBuildSpan.traceChild('load-next-config').traceAsyncFn(()=>(0, _turborepoaccesstrace.turborepoTraceAccess)(()=>(0, _config.default)(_constants1.PHASE_PRODUCTION_BUILD, dir, {
                        // Log for next.config loading process
                        silent: false,
                        reactProductionProfiling,
                        debugPrerender
                    }), turborepoAccessTraceResult));
            loadedConfig = config;
            // Reading the config can modify environment variables that influence the bundler selection.
            bundler = (0, _bundler.finalizeBundlerFromConfig)(bundler);
            nextBuildSpan.setAttribute('bundler', getBundlerForTelemetry(bundler));
            // Install the native bindings early so we can have synchronous access later.
            await (0, _installbindings.installBindings)((_config_experimental = config.experimental) == null ? void 0 : _config_experimental.useWasmBinary);
            process.env.NEXT_DEPLOYMENT_ID = config.deploymentId || '';
            _buildcontext.NextBuildContext.config = config;
            let configOutDir = 'out';
            if ((0, _utils2.hasCustomExportOutput)(config)) {
                configOutDir = config.distDir;
                config.distDir = '.next';
            }
            const distDir = _path.default.join(dir, config.distDir);
            _buildcontext.NextBuildContext.distDir = distDir;
            (0, _trace.setGlobal)('phase', _constants1.PHASE_PRODUCTION_BUILD);
            (0, _trace.setGlobal)('distDir', distDir);
            const buildId = await getBuildId(isGenerateMode, distDir, nextBuildSpan, config);
            _buildcontext.NextBuildContext.buildId = buildId;
            if (experimentalBuildMode === 'generate-env') {
                if (bundler === _bundler.Bundler.Turbopack) {
                    _log.warn('generate-env is not needed with turbopack');
                    process.exit(0);
                }
                _log.info('Inlining static env ...');
                await nextBuildSpan.traceChild('inline-static-env').traceAsyncFn(async ()=>{
                    await (0, _inlinestaticenv.inlineStaticEnv)({
                        distDir,
                        config
                    });
                });
                _log.info('Complete');
                await (0, _trace.flushAllTraces)();
                (0, _swc.teardownTraceSubscriber)();
                process.exit(0);
            }
            // when using compile mode static env isn't inlined so we
            // need to populate in normal runtime env
            if (isCompileMode || isGenerateMode) {
                (0, _staticenv.populateStaticEnv)(config, config.deploymentId);
            }
            const customRoutes = await nextBuildSpan.traceChild('load-custom-routes').traceAsyncFn(()=>(0, _loadcustomroutes.default)(config));
            const { headers, rewrites, redirects } = customRoutes;
            const combinedRewrites = [
                ...rewrites.beforeFiles,
                ...rewrites.afterFiles,
                ...rewrites.fallback
            ];
            const hasRewrites = combinedRewrites.length > 0;
            _buildcontext.NextBuildContext.hasRewrites = hasRewrites;
            _buildcontext.NextBuildContext.originalRewrites = config._originalRewrites;
            _buildcontext.NextBuildContext.originalRedirects = config._originalRedirects;
            const distDirCreated = await nextBuildSpan.traceChild('create-dist-dir').traceAsyncFn(async ()=>{
                try {
                    await _fs.promises.mkdir(distDir, {
                        recursive: true
                    });
                    return true;
                } catch (err) {
                    if ((0, _iserror.default)(err) && err.code === 'EPERM') {
                        return false;
                    }
                    throw err;
                }
            });
            if (!distDirCreated || !await (0, _iswriteable.isWriteable)(distDir)) {
                throw Object.defineProperty(new Error('> Build directory is not writeable. https://nextjs.org/docs/messages/build-dir-not-writeable'), "__NEXT_ERROR_CODE", {
                    value: "E202",
                    enumerable: false,
                    configurable: true
                });
            }
            if (config.experimental.lockDistDir) {
                // This leaks the lock file descriptor. That's okay, it'll be cleaned up by the OS upon
                // process exit.
                await _lockfile.Lockfile.acquireWithRetriesOrExit(_path.default.join(distDir, 'lock'), 'next build');
            }
            if (config.cleanDistDir && !isGenerateMode) {
                await nextBuildSpan.traceChild('clean').traceAsyncFn(()=>(0, _recursivedelete.recursiveDeleteSyncWithAsyncRetries)(distDir, /^(cache|dev|lock)/));
            }
            const cacheDir = getCacheDir(distDir);
            const telemetry = new _storage.Telemetry({
                distDir
            });
            (0, _trace.setGlobal)('telemetry', telemetry);
            const publicDir = _path.default.join(dir, 'public');
            const { pagesDir, appDir } = (0, _findpagesdir.findPagesDir)(dir);
            if (pagesDir && appDir) {
                appType = 'hybrid';
            } else if (pagesDir) {
                appType = 'pages';
            } else if (appDir) {
                appType = 'app';
            }
            if (!appDirOnly && !pagesDir) {
                appDirOnly = true;
            }
            _buildcontext.NextBuildContext.pagesDir = pagesDir;
            _buildcontext.NextBuildContext.appDir = appDir;
            const enabledDirectories = {
                app: typeof appDir === 'string',
                pages: typeof pagesDir === 'string'
            };
            // Generate a random encryption key for this build.
            // This key is used to encrypt cross boundary values and can be used to generate hashes.
            const encryptionKey = await (0, _encryptionutilsserver.generateEncryptionKeyBase64)({
                isBuild: true,
                distDir
            });
            _buildcontext.NextBuildContext.encryptionKey = encryptionKey;
            const isSrcDir = _path.default.relative(dir, pagesDir || appDir || '').startsWith('src');
            const hasPublicDir = (0, _fs.existsSync)(publicDir);
            telemetry.record((0, _events.eventCliSession)(config, {
                webpackVersion: 5,
                cliCommand: 'build',
                isSrcDir,
                hasNowJson: !!await (0, _findup.default)('now.json', {
                    cwd: dir
                }),
                isCustomServer: null,
                turboFlag: false,
                pagesDir: !!pagesDir,
                appDir: !!appDir
            }));
            (0, _events.eventNextPlugins)(_path.default.resolve(dir)).then((events)=>telemetry.record(events));
            (0, _swcplugins.eventSwcPlugins)(_path.default.resolve(dir), config).then((events)=>telemetry.record(events));
            // Always log next version first then start rest jobs
            const { envInfo, experimentalFeatures, cacheComponents } = await (0, _appinfolog.getStartServerInfo)({
                dir,
                dev: false,
                debugPrerender
            });
            (0, _appinfolog.logStartInfo)({
                networkUrl: null,
                appUrl: null,
                envInfo,
                experimentalFeatures,
                logBundler: true,
                cacheComponents
            });
            const typeCheckingOptions = {
                dir,
                appDir,
                pagesDir,
                telemetry,
                nextBuildSpan,
                config,
                cacheDir,
                debugBuildPaths: debugBuildAppPaths !== undefined || debugBuildPagePaths !== undefined ? {
                    app: debugBuildAppPaths,
                    pages: debugBuildPagePaths
                } : undefined
            };
            if (appDir && 'exportPathMap' in config) {
                const errorMessage = 'The "exportPathMap" configuration cannot be used with the "app" directory. Please use generateStaticParams() instead.';
                _log.error(errorMessage);
                await telemetry.flush();
                throw Object.defineProperty(new Error(errorMessage), "__NEXT_ERROR_CODE", {
                    value: "E394",
                    enumerable: false,
                    configurable: true
                });
            }
            const validFileMatcher = (0, _findpagefile.createValidFileMatcher)(config.pageExtensions, appDir);
            const providedPagePaths = JSON.parse(process.env.NEXT_PRIVATE_PAGE_PATHS || '[]');
            let pagesPaths = Boolean(process.env.NEXT_PRIVATE_PAGE_PATHS) ? providedPagePaths : !appDirOnly && pagesDir ? await nextBuildSpan.traceChild('collect-pages').traceAsyncFn(()=>(0, _entries.collectPagesFiles)(pagesDir, validFileMatcher)) : [];
            // Apply debug build paths filter if specified
            // If debugBuildPagePaths is defined (even if empty), only build specified pages
            if (debugBuildPagePaths !== undefined) {
                if (debugBuildPagePaths.length > 0) {
                    const debugPathsSet = new Set(debugBuildPagePaths);
                    pagesPaths = pagesPaths.filter((pagePath)=>debugPathsSet.has(pagePath));
                } else {
                    // Empty array means build no pages
                    pagesPaths = [];
                }
            }
            const middlewareDetectionRegExp = new RegExp(`^${_constants.MIDDLEWARE_FILENAME}\\.(?:${config.pageExtensions.join('|')})$`);
            const proxyDetectionRegExp = new RegExp(`^${_constants.PROXY_FILENAME}\\.(?:${config.pageExtensions.join('|')})$`);
            const instrumentationHookDetectionRegExp = new RegExp(`^${_constants.INSTRUMENTATION_HOOK_FILENAME}\\.(?:${config.pageExtensions.join('|')})$`);
            const rootDir = _path.default.join(pagesDir || appDir, '..');
            const includes = [
                middlewareDetectionRegExp,
                proxyDetectionRegExp,
                instrumentationHookDetectionRegExp
            ];
            const rootPaths = Array.from(await (0, _getfilesindir.getFilesInDir)(rootDir)).filter((file)=>includes.some((include)=>include.test(file))).sort((0, _sortbypageexts.sortByPageExts)(config.pageExtensions)).map((file)=>_path.default.join(rootDir, file).replace(dir, ''));
            let instrumentationHookFilePath;
            let proxyFilePath;
            let middlewareFilePath;
            for (const rootPath of rootPaths){
                const { name: fileBaseName, dir: fileDir } = _path.default.parse(rootPath);
                const normalizedFileDir = (0, _normalizepathsep.normalizePathSep)(fileDir);
                const isAtConventionLevel = normalizedFileDir === '/' || normalizedFileDir === '/src';
                if (isAtConventionLevel && fileBaseName === _constants.MIDDLEWARE_FILENAME) {
                    middlewareFilePath = rootPath;
                }
                if (isAtConventionLevel && fileBaseName === _constants.PROXY_FILENAME) {
                    proxyFilePath = rootPath;
                }
                if (isAtConventionLevel && fileBaseName === _constants.INSTRUMENTATION_HOOK_FILENAME) {
                    instrumentationHookFilePath = rootPath;
                }
            }
            if (middlewareFilePath) {
                if (proxyFilePath) {
                    const cwd = process.cwd();
                    const absoluteProxyPath = _path.default.join(rootDir, proxyFilePath);
                    const absoluteMiddlewarePath = _path.default.join(rootDir, middlewareFilePath);
                    throw Object.defineProperty(new Error(`Both ${_constants.MIDDLEWARE_FILENAME} file "./${_path.default.relative(cwd, absoluteMiddlewarePath)}" and ${_constants.PROXY_FILENAME} file "./${_path.default.relative(cwd, absoluteProxyPath)}" are detected. Please use "./${_path.default.relative(cwd, absoluteProxyPath)}" only. Learn more: https://nextjs.org/docs/messages/middleware-to-proxy`), "__NEXT_ERROR_CODE", {
                        value: "E900",
                        enumerable: false,
                        configurable: true
                    });
                }
                _log.warnOnce(`The "${_constants.MIDDLEWARE_FILENAME}" file convention is deprecated. Please use "${_constants.PROXY_FILENAME}" instead. Learn more: https://nextjs.org/docs/messages/middleware-to-proxy`);
            }
            const hasInstrumentationHook = Boolean(instrumentationHookFilePath);
            const hasMiddlewareFile = Boolean(middlewareFilePath);
            const hasProxyFile = Boolean(proxyFilePath);
            _buildcontext.NextBuildContext.hasInstrumentationHook = hasInstrumentationHook;
            const previewProps = await (0, _previewkeyutils.generatePreviewKeys)({
                isBuild: true,
                distDir
            });
            _buildcontext.NextBuildContext.previewProps = previewProps;
            const mappedPages = await nextBuildSpan.traceChild('create-pages-mapping').traceAsyncFn(()=>(0, _entries.createPagesMapping)({
                    isDev: false,
                    pageExtensions: config.pageExtensions,
                    pagesType: _pagetypes.PAGE_TYPES.PAGES,
                    pagePaths: pagesPaths,
                    pagesDir,
                    appDir,
                    appDirOnly
                }));
            _buildcontext.NextBuildContext.mappedPages = mappedPages;
            // Update appDirOnly if no user pages routes are found
            if (Object.keys(mappedPages).length === 0 && !appDirOnly) {
                _buildcontext.NextBuildContext.appDirOnly = appDirOnly = true;
            }
            let mappedAppPages;
            let mappedAppLayouts;
            let denormalizedAppPages;
            if (appDir) {
                const providedAppPaths = JSON.parse(process.env.NEXT_PRIVATE_APP_PATHS || '[]');
                let appPaths;
                let layoutPaths;
                if (Boolean(process.env.NEXT_PRIVATE_APP_PATHS)) {
                    // used for testing
                    appPaths = providedAppPaths;
                    layoutPaths = [];
                } else {
                    // Collect app pages, layouts, and default files in a single directory traversal
                    const result = await nextBuildSpan.traceChild('collect-app-files').traceAsyncFn(()=>(0, _entries.collectAppFiles)(appDir, validFileMatcher));
                    appPaths = result.appPaths;
                    layoutPaths = result.layoutPaths;
                    // Apply debug build paths filter if specified
                    // If debugBuildAppPaths is defined (even if empty), only build specified app paths
                    if (debugBuildAppPaths !== undefined) {
                        if (debugBuildAppPaths.length > 0) {
                            const debugPathsSet = new Set(debugBuildAppPaths);
                            appPaths = appPaths.filter((appPath)=>debugPathsSet.has(appPath));
                        } else {
                            // Empty array means build no app paths
                            appPaths = [];
                        }
                    }
                // Note: defaultPaths are not used in the build process, only for slot detection in generating route types
                }
                mappedAppPages = await nextBuildSpan.traceChild('create-app-mapping').traceAsyncFn(()=>(0, _entries.createPagesMapping)({
                        pagePaths: appPaths,
                        isDev: false,
                        pagesType: _pagetypes.PAGE_TYPES.APP,
                        pageExtensions: config.pageExtensions,
                        pagesDir,
                        appDir,
                        appDirOnly
                    }));
                mappedAppLayouts = await nextBuildSpan.traceChild('create-app-layouts').traceAsyncFn(()=>(0, _entries.createPagesMapping)({
                        pagePaths: layoutPaths,
                        isDev: false,
                        pagesType: _pagetypes.PAGE_TYPES.APP,
                        pageExtensions: config.pageExtensions,
                        pagesDir,
                        appDir,
                        appDirOnly
                    }));
                _buildcontext.NextBuildContext.mappedAppPages = mappedAppPages;
            }
            const mappedRootPaths = await (0, _entries.createPagesMapping)({
                isDev: false,
                pageExtensions: config.pageExtensions,
                pagePaths: rootPaths,
                pagesType: _pagetypes.PAGE_TYPES.ROOT,
                pagesDir: pagesDir,
                appDir,
                appDirOnly
            });
            _buildcontext.NextBuildContext.mappedRootPaths = mappedRootPaths;
            const pagesPageKeys = Object.keys(mappedPages);
            const conflictingAppPagePaths = [];
            const appPageKeys = new Set();
            if (mappedAppPages) {
                denormalizedAppPages = Object.keys(mappedAppPages);
                for (const appKey of denormalizedAppPages){
                    const normalizedAppPageKey = (0, _apppaths.normalizeAppPath)(appKey);
                    const pagePath = mappedPages[normalizedAppPageKey];
                    if (pagePath) {
                        const appPath = mappedAppPages[appKey];
                        conflictingAppPagePaths.push([
                            pagePath.replace(/^private-next-pages/, 'pages'),
                            appPath.replace(/^private-next-app-dir/, 'app')
                        ]);
                    }
                    appPageKeys.add(normalizedAppPageKey);
                }
            }
            const appPaths = Array.from(appPageKeys);
            // Validate that the app paths are valid. This is currently duplicating
            // the logic from packages/next/src/shared/lib/router/utils/sorted-routes.ts
            // but is instead specifically focused on code that can be shared
            // eventually with the development code.
            (0, _validateapppaths.validateAppPaths)(appPaths);
            // Interception routes are modelled as beforeFiles rewrites
            rewrites.beforeFiles.push(...(0, _generateinterceptionroutesrewrites.generateInterceptionRoutesRewrites)(appPaths, config.basePath));
            _buildcontext.NextBuildContext.rewrites = rewrites;
            const totalAppPagesCount = appPaths.length;
            const pageKeys = {
                pages: pagesPageKeys,
                app: appPaths.length > 0 ? appPaths : undefined
            };
            await nextBuildSpan.traceChild('generate-route-types').traceAsyncFn(async ()=>{
                const routeTypesFilePath = _path.default.join(distDir, 'types', 'routes.d.ts');
                const validatorFilePath = _path.default.join(distDir, 'types', 'validator.ts');
                await (0, _promises.mkdir)(_path.default.dirname(routeTypesFilePath), {
                    recursive: true
                });
                let appRoutes = [];
                let appRouteHandlers = [];
                let layoutRoutes = [];
                let slots = [];
                const { pageRoutes, pageApiRoutes } = (0, _entries.processPageRoutes)(mappedPages, dir, isSrcDir);
                // Build app routes
                if (appDir && mappedAppPages) {
                    // Extract slots from both pages and default files
                    const slotsFromPages = (0, _entries.extractSlotsFromAppRoutes)(mappedAppPages);
                    let slotsFromDefaults = [];
                    // Collect and map default files for slot extraction
                    const { defaultPaths } = await nextBuildSpan.traceChild('collect-default-files').traceAsyncFn(()=>(0, _entries.collectAppFiles)(appDir, validFileMatcher));
                    if (defaultPaths.length > 0) {
                        const mappedDefaultFiles = await nextBuildSpan.traceChild('create-default-mapping').traceAsyncFn(()=>(0, _entries.createPagesMapping)({
                                pagePaths: defaultPaths,
                                isDev: false,
                                pagesType: _pagetypes.PAGE_TYPES.APP,
                                pageExtensions: config.pageExtensions,
                                pagesDir,
                                appDir,
                                appDirOnly
                            }));
                        slotsFromDefaults = (0, _entries.extractSlotsFromDefaultFiles)(mappedDefaultFiles);
                    }
                    // Combine slots and deduplicate using Set
                    slots = (0, _entries.combineSlots)(slotsFromPages, slotsFromDefaults);
                    const result = (0, _entries.processAppRoutes)(mappedAppPages, validFileMatcher, dir, isSrcDir);
                    appRoutes = result.appRoutes;
                    appRouteHandlers = result.appRouteHandlers;
                }
                // Build app layouts
                if (appDir && mappedAppLayouts) {
                    layoutRoutes = (0, _entries.processLayoutRoutes)(mappedAppLayouts, dir, isSrcDir);
                }
                const routeTypesManifest = await (0, _routetypesutils.createRouteTypesManifest)({
                    dir,
                    pageRoutes,
                    appRoutes,
                    appRouteHandlers,
                    pageApiRoutes,
                    layoutRoutes,
                    slots,
                    redirects: config.redirects,
                    rewrites: config.rewrites,
                    validatorFilePath
                });
                await (0, _routetypesutils.writeRouteTypesManifest)(routeTypesManifest, routeTypesFilePath, config);
                await (0, _routetypesutils.writeValidatorFile)(routeTypesManifest, validatorFilePath);
            });
            // Turbopack already handles conflicting app and page routes.
            if (bundler !== _bundler.Bundler.Turbopack) {
                const numConflictingAppPaths = conflictingAppPagePaths.length;
                if (mappedAppPages && numConflictingAppPaths > 0) {
                    const errorMessage = `Conflicting app and page file${numConflictingAppPaths === 1 ? ' was' : 's were'} found, please remove the conflicting files to continue:`;
                    _log.error(errorMessage);
                    for (const [pagePath, appPath] of conflictingAppPagePaths){
                        _log.error(`  "${pagePath}" - "${appPath}"`);
                    }
                    await telemetry.flush();
                    throw Object.defineProperty(new Error(errorMessage), "__NEXT_ERROR_CODE", {
                        value: "E394",
                        enumerable: false,
                        configurable: true
                    });
                }
            }
            const conflictingPublicFiles = [];
            const hasPages404 = (_mappedPages_404 = mappedPages['/404']) == null ? void 0 : _mappedPages_404.startsWith(_constants.PAGES_DIR_ALIAS);
            const hasApp404 = !!(mappedAppPages == null ? void 0 : mappedAppPages[_entryconstants.UNDERSCORE_NOT_FOUND_ROUTE_ENTRY]);
            const hasAppGlobalError = !!(mappedAppPages == null ? void 0 : mappedAppPages[_entryconstants.UNDERSCORE_GLOBAL_ERROR_ROUTE_ENTRY]);
            const hasCustomErrorPage = (_mappedPages__error = mappedPages['/_error']) == null ? void 0 : _mappedPages__error.startsWith(_constants.PAGES_DIR_ALIAS);
            // Check if there are any user pages (non-reserved pages) in the pages router
            const hasUserPagesRoutes = Object.keys(mappedPages).some((route)=>!(0, _utils1.isReservedPage)(route));
            if (hasPublicDir) {
                const hasPublicUnderScoreNextDir = (0, _fs.existsSync)(_path.default.join(publicDir, '_next'));
                if (hasPublicUnderScoreNextDir) {
                    throw Object.defineProperty(new Error(_constants.PUBLIC_DIR_MIDDLEWARE_CONFLICT), "__NEXT_ERROR_CODE", {
                        value: "E394",
                        enumerable: false,
                        configurable: true
                    });
                }
            }
            await nextBuildSpan.traceChild('public-dir-conflict-check').traceAsyncFn(async ()=>{
                // Check if pages conflict with files in `public`
                // Only a page of public file can be served, not both.
                for(const page in mappedPages){
                    const hasPublicPageFile = await (0, _fileexists.fileExists)(_path.default.join(publicDir, page === '/' ? '/index' : page), _fileexists.FileType.File);
                    if (hasPublicPageFile) {
                        conflictingPublicFiles.push(page);
                    }
                }
                const numConflicting = conflictingPublicFiles.length;
                if (numConflicting) {
                    throw Object.defineProperty(new Error(`Conflicting public and page file${numConflicting === 1 ? ' was' : 's were'} found. https://nextjs.org/docs/messages/conflicting-public-file-page\n${conflictingPublicFiles.join('\n')}`), "__NEXT_ERROR_CODE", {
                        value: "E270",
                        enumerable: false,
                        configurable: true
                    });
                }
            });
            const nestedReservedPages = pageKeys.pages.filter((page)=>{
                return page.match(/\/(_app|_document|_error)$/) && _path.default.dirname(page) !== '/';
            });
            if (nestedReservedPages.length) {
                _log.warn(`The following reserved Next.js pages were detected not directly under the pages directory:\n` + nestedReservedPages.join('\n') + `\nSee more info here: https://nextjs.org/docs/messages/nested-reserved-page\n`);
            }
            const restrictedRedirectPaths = [
                '/_next'
            ].map((p)=>config.basePath ? `${config.basePath}${p}` : p);
            const isAppCacheComponentsEnabled = Boolean(config.cacheComponents);
            const isAuthInterruptsEnabled = Boolean(config.experimental.authInterrupts);
            const isAppPPREnabled = (0, _ppr.checkIsAppPPREnabled)(config.experimental.ppr);
            const routesManifestPath = _path.default.join(distDir, _constants1.ROUTES_MANIFEST);
            // Generate the routes manifest using the extracted helper
            const { routesManifest, dynamicRoutes, sourcePages } = nextBuildSpan.traceChild('generate-routes-manifest').traceFn(()=>(0, _generateroutesmanifest.generateRoutesManifest)({
                    appType,
                    pageKeys,
                    config,
                    redirects,
                    headers,
                    rewrites,
                    restrictedRedirectPaths,
                    isAppPPREnabled
                }));
            // For pages directory, we run type checking after route collection but before build.
            if (!appDir && !isCompileMode) {
                await (0, _typecheck.startTypeChecking)(typeCheckingOptions);
            }
            let clientRouterFilters;
            if (config.experimental.clientRouterFilter) {
                const nonInternalRedirects = (config._originalRedirects || []).filter((r)=>!r.internal);
                clientRouterFilters = (0, _createclientrouterfilter.createClientRouterFilter)([
                    ...appPaths
                ], config.experimental.clientRouterFilterRedirects ? nonInternalRedirects : [], config.experimental.clientRouterFilterAllowedRate);
                _buildcontext.NextBuildContext.clientRouterFilters = clientRouterFilters;
            }
            // Ensure commonjs handling is used for files in the distDir (generally .next)
            // Files outside of the distDir can be "type": "module"
            await writeFileUtf8(_path.default.join(distDir, 'package.json'), '{"type": "commonjs"}');
            // These are written to distDir, so they need to come after creating and cleaning distDr.
            await (0, _builddiagnostics.recordFrameworkVersion)("16.1.6");
            await (0, _builddiagnostics.updateBuildDiagnostics)({
                buildStage: 'start'
            });
            const outputFileTracingRoot = config.outputFileTracingRoot || dir;
            const pagesManifestPath = _path.default.join(distDir, _constants1.SERVER_DIRECTORY, _constants1.PAGES_MANIFEST);
            let buildTraceContext;
            let buildTracesPromise = undefined;
            // If there's has a custom webpack config and disable the build worker.
            // Otherwise respect the option if it's set.
            const useBuildWorker = config.experimental.webpackBuildWorker || config.experimental.webpackBuildWorker === undefined && !config.webpack;
            const runServerAndEdgeInParallel = config.experimental.parallelServerCompiles;
            const collectServerBuildTracesInParallel = config.experimental.parallelServerBuildTraces || config.experimental.parallelServerBuildTraces === undefined && isCompileMode;
            nextBuildSpan.setAttribute('has-custom-webpack-config', String(!!config.webpack));
            nextBuildSpan.setAttribute('use-build-worker', String(useBuildWorker));
            if (!useBuildWorker && (runServerAndEdgeInParallel || collectServerBuildTracesInParallel)) {
                throw Object.defineProperty(new Error('The "parallelServerBuildTraces" and "parallelServerCompiles" options may only be used when build workers can be used. Read more: https://nextjs.org/docs/messages/parallel-build-without-worker'), "__NEXT_ERROR_CODE", {
                    value: "E101",
                    enumerable: false,
                    configurable: true
                });
            }
            _log.info('Creating an optimized production build ...');
            (0, _trace1.traceMemoryUsage)('Starting build', nextBuildSpan);
            await (0, _builddiagnostics.updateBuildDiagnostics)({
                buildStage: 'compile',
                buildOptions: {
                    useBuildWorker: String(useBuildWorker)
                }
            });
            let shutdownPromise = Promise.resolve();
            if (!isGenerateMode) {
                if (bundler === _bundler.Bundler.Turbopack) {
                    const { duration: compilerDuration, shutdownPromise: p, ...rest } = await (0, _turbopackbuild.turbopackBuild)(process.env.NEXT_TURBOPACK_USE_WORKER === undefined || process.env.NEXT_TURBOPACK_USE_WORKER !== '0');
                    shutdownPromise = p;
                    (0, _trace1.traceMemoryUsage)('Finished build', nextBuildSpan);
                    buildTraceContext = rest.buildTraceContext;
                    const durationString = (0, _durationtostring.durationToString)(compilerDuration);
                    _log.event(`Compiled successfully in ${durationString}`);
                    telemetry.record((0, _events.eventBuildCompleted)(pagesPaths, {
                        bundler: 'turbopack',
                        durationInSeconds: Math.round(compilerDuration),
                        totalAppPagesCount
                    }));
                } else {
                    if (runServerAndEdgeInParallel || collectServerBuildTracesInParallel) {
                        let durationInSeconds = 0;
                        await (0, _builddiagnostics.updateBuildDiagnostics)({
                            buildStage: 'compile-server'
                        });
                        const serverBuildPromise = (0, _webpackbuild.webpackBuild)(useBuildWorker, [
                            'server'
                        ]).then((res)=>{
                            (0, _trace1.traceMemoryUsage)('Finished server compilation', nextBuildSpan);
                            buildTraceContext = res.buildTraceContext;
                            durationInSeconds += res.duration;
                            if (collectServerBuildTracesInParallel) {
                                const buildTraceWorker = new _worker.Worker(require.resolve('./collect-build-traces'), {
                                    debuggerPortOffset: -1,
                                    isolatedMemory: false,
                                    numWorkers: 1,
                                    exposedMethods: [
                                        'collectBuildTraces'
                                    ]
                                });
                                buildTracesPromise = nextBuildSpan.traceChild('collect-build-traces').traceAsyncFn(()=>{
                                    return buildTraceWorker.collectBuildTraces({
                                        dir,
                                        config,
                                        distDir,
                                        // Serialize Map as this is sent to the worker.
                                        edgeRuntimeRoutes: (0, _utils1.collectRoutesUsingEdgeRuntime)(new Map()),
                                        staticPages: [],
                                        buildTraceContext,
                                        outputFileTracingRoot
                                    }).catch((err)=>{
                                        console.error(err);
                                        throw err;
                                    });
                                });
                            }
                        });
                        if (!runServerAndEdgeInParallel) {
                            await serverBuildPromise;
                            await (0, _builddiagnostics.updateBuildDiagnostics)({
                                buildStage: 'webpack-compile-edge-server'
                            });
                        }
                        const edgeBuildPromise = (0, _webpackbuild.webpackBuild)(useBuildWorker, [
                            'edge-server'
                        ]).then((res)=>{
                            durationInSeconds += res.duration;
                            (0, _trace1.traceMemoryUsage)('Finished edge-server compilation', nextBuildSpan);
                        });
                        if (runServerAndEdgeInParallel) {
                            await serverBuildPromise;
                            await (0, _builddiagnostics.updateBuildDiagnostics)({
                                buildStage: 'webpack-compile-edge-server'
                            });
                        }
                        await edgeBuildPromise;
                        await (0, _builddiagnostics.updateBuildDiagnostics)({
                            buildStage: 'webpack-compile-client'
                        });
                        await (0, _webpackbuild.webpackBuild)(useBuildWorker, [
                            'client'
                        ]).then((res)=>{
                            durationInSeconds += res.duration;
                            (0, _trace1.traceMemoryUsage)('Finished client compilation', nextBuildSpan);
                        });
                        const durationString = (0, _durationtostring.durationToString)(durationInSeconds);
                        _log.event(`Compiled successfully in ${durationString}`);
                        telemetry.record((0, _events.eventBuildCompleted)(pagesPaths, {
                            bundler: getBundlerForTelemetry(bundler),
                            durationInSeconds,
                            totalAppPagesCount
                        }));
                    } else {
                        const { duration: compilerDuration, ...rest } = await (0, _webpackbuild.webpackBuild)(useBuildWorker, null);
                        (0, _trace1.traceMemoryUsage)('Finished build', nextBuildSpan);
                        buildTraceContext = rest.buildTraceContext;
                        telemetry.record((0, _events.eventBuildCompleted)(pagesPaths, {
                            bundler: getBundlerForTelemetry(bundler),
                            durationInSeconds: compilerDuration,
                            totalAppPagesCount
                        }));
                    }
                }
                await (0, _afterproductioncompile.runAfterProductionCompile)({
                    config,
                    buildSpan: nextBuildSpan,
                    telemetry,
                    metadata: {
                        projectDir: dir,
                        distDir
                    }
                });
            }
            // For app directory, we run type checking after build.
            if (appDir && !isCompileMode && !isGenerateMode) {
                await (0, _builddiagnostics.updateBuildDiagnostics)({
                    buildStage: 'type-checking'
                });
                await (0, _typecheck.startTypeChecking)(typeCheckingOptions);
                (0, _trace1.traceMemoryUsage)('Finished type checking', nextBuildSpan);
            }
            const requiredServerFilesManifest = await nextBuildSpan.traceChild('generate-required-server-files').traceAsyncFn(async ()=>{
                let runtimeConfig = (0, _configshared.getNextConfigRuntime)(config);
                const normalizedCacheHandlers = {};
                for (const [key, value] of Object.entries(runtimeConfig.cacheHandlers || {})){
                    if (key && value) {
                        normalizedCacheHandlers[key] = _path.default.relative(distDir, value);
                    }
                }
                // getNextConfigRuntime only filters if experimental.runtimeServerDeploymentId is true,
                // but we unconditionally want to remove configFile for this manifest
                let runtimeConfigWithoutFilePath = {
                    ...runtimeConfig
                };
                delete runtimeConfigWithoutFilePath.configFile;
                const serverFilesManifest = {
                    version: 1,
                    config: {
                        ...runtimeConfigWithoutFilePath,
                        ..._ciinfo.hasNextSupport ? {
                            compress: false
                        } : {},
                        cacheHandler: runtimeConfig.cacheHandler ? _path.default.relative(distDir, runtimeConfig.cacheHandler) : runtimeConfig.cacheHandler,
                        cacheHandlers: normalizedCacheHandlers,
                        experimental: {
                            ...runtimeConfig.experimental,
                            trustHostHeader: _ciinfo.hasNextSupport,
                            isExperimentalCompile: isCompileMode
                        }
                    },
                    appDir: dir,
                    relativeAppDir: _path.default.relative(outputFileTracingRoot, dir),
                    files: [
                        _constants1.ROUTES_MANIFEST,
                        _path.default.relative(distDir, pagesManifestPath),
                        _constants1.BUILD_MANIFEST,
                        _constants1.PRERENDER_MANIFEST,
                        _path.default.join(_constants1.SERVER_DIRECTORY, _constants1.FUNCTIONS_CONFIG_MANIFEST),
                        _path.default.join(_constants1.SERVER_DIRECTORY, _constants1.MIDDLEWARE_MANIFEST),
                        _path.default.join(_constants1.SERVER_DIRECTORY, _constants1.MIDDLEWARE_BUILD_MANIFEST + '.js'),
                        ...bundler !== _bundler.Bundler.Turbopack ? [
                            _path.default.join(_constants1.SERVER_DIRECTORY, _constants1.MIDDLEWARE_REACT_LOADABLE_MANIFEST + '.js'),
                            _constants1.REACT_LOADABLE_MANIFEST
                        ] : [],
                        ...appDir ? [
                            ...config.experimental.sri ? [
                                _path.default.join(_constants1.SERVER_DIRECTORY, _constants1.SUBRESOURCE_INTEGRITY_MANIFEST + '.js'),
                                _path.default.join(_constants1.SERVER_DIRECTORY, _constants1.SUBRESOURCE_INTEGRITY_MANIFEST + '.json')
                            ] : [],
                            _path.default.join(_constants1.SERVER_DIRECTORY, _constants1.APP_PATHS_MANIFEST),
                            _path.default.join(_constants1.APP_PATH_ROUTES_MANIFEST),
                            _path.default.join(_constants1.SERVER_DIRECTORY, _constants1.SERVER_REFERENCE_MANIFEST + '.js'),
                            _path.default.join(_constants1.SERVER_DIRECTORY, _constants1.SERVER_REFERENCE_MANIFEST + '.json')
                        ] : [],
                        ...pagesDir && bundler !== _bundler.Bundler.Turbopack ? [
                            _constants1.DYNAMIC_CSS_MANIFEST + '.json',
                            _path.default.join(_constants1.SERVER_DIRECTORY, _constants1.DYNAMIC_CSS_MANIFEST + '.js')
                        ] : [],
                        _constants1.BUILD_ID_FILE,
                        _path.default.join(_constants1.SERVER_DIRECTORY, _constants1.NEXT_FONT_MANIFEST + '.js'),
                        _path.default.join(_constants1.SERVER_DIRECTORY, _constants1.NEXT_FONT_MANIFEST + '.json'),
                        _constants1.SERVER_FILES_MANIFEST + '.json'
                    ].filter(_nonnullable.nonNullable).map((file)=>_path.default.join(config.distDir, file)),
                    ignore: []
                };
                if (hasInstrumentationHook) {
                    serverFilesManifest.files.push(_path.default.join(config.distDir, _constants1.SERVER_DIRECTORY, `${_constants.INSTRUMENTATION_HOOK_FILENAME}.js`));
                    // If there are edge routes, append the edge instrumentation hook
                    // Turbopack generates this chunk with a hashed name and references it in middleware-manifest.
                    let edgeInstrumentationHook = _path.default.join(config.distDir, _constants1.SERVER_DIRECTORY, `edge-${_constants.INSTRUMENTATION_HOOK_FILENAME}.js`);
                    if (bundler !== _bundler.Bundler.Turbopack && (0, _fs.existsSync)(_path.default.join(dir, edgeInstrumentationHook))) {
                        serverFilesManifest.files.push(edgeInstrumentationHook);
                    }
                }
                if (config.experimental.optimizeCss) {
                    const globOrig = require('next/dist/compiled/glob');
                    const cssFilePaths = await new Promise((resolve, reject)=>{
                        globOrig('**/*.css', {
                            cwd: _path.default.join(distDir, 'static')
                        }, (err, files)=>{
                            if (err) {
                                return reject(err);
                            }
                            resolve(files);
                        });
                    });
                    serverFilesManifest.files.push(...cssFilePaths.map((filePath)=>_path.default.join(config.distDir, 'static', filePath)));
                }
                // Under standalone mode, we need to ensure that the cache entry debug
                // handler is copied so that it can be used in the test. This is required
                // for the turbopack test to run as it's more strict about the build
                // directories. This is only used for testing and is not used in
                // production.
                if (process.env.__NEXT_TEST_MODE && process.env.NEXT_PRIVATE_DEBUG_CACHE_ENTRY_HANDLERS) {
                    serverFilesManifest.files.push(_path.default.relative(dir, _path.default.isAbsolute(process.env.NEXT_PRIVATE_DEBUG_CACHE_ENTRY_HANDLERS) ? process.env.NEXT_PRIVATE_DEBUG_CACHE_ENTRY_HANDLERS : _path.default.join(dir, process.env.NEXT_PRIVATE_DEBUG_CACHE_ENTRY_HANDLERS)));
                }
                return serverFilesManifest;
            });
            await writeRequiredServerFilesManifest(distDir, requiredServerFilesManifest);
            const numberOfWorkers = getNumberOfWorkers(config);
            const collectingPageDataStart = process.hrtime();
            const postCompileSpinner = (0, _spinner.default)(`Collecting page data using ${numberOfWorkers} worker${numberOfWorkers > 1 ? 's' : ''}`);
            const buildManifestPath = _path.default.join(distDir, _constants1.BUILD_MANIFEST);
            let staticAppPagesCount = 0;
            let serverAppPagesCount = 0;
            let edgeRuntimeAppCount = 0;
            let edgeRuntimePagesCount = 0;
            const ssgPages = new Set();
            const ssgStaticFallbackPages = new Set();
            const ssgBlockingFallbackPages = new Set();
            const staticPages = new Set();
            const invalidPages = new Set();
            const serverPropsPages = new Set();
            const additionalPaths = new Map();
            const staticPaths = new Map();
            const appNormalizedPaths = new Map();
            const fallbackModes = new Map();
            const appDefaultConfigs = new Map();
            const pageInfos = new Map();
            let pagesManifest = await readManifest(pagesManifestPath);
            const buildManifest = await readManifest(buildManifestPath);
            const appPathRoutes = {};
            if (appDir) {
                const appPathsManifest = await readManifest(_path.default.join(distDir, _constants1.SERVER_DIRECTORY, _constants1.APP_PATHS_MANIFEST));
                for(const key in appPathsManifest){
                    appPathRoutes[key] = (0, _apppaths.normalizeAppPath)(key);
                }
                await writeManifest(_path.default.join(distDir, _constants1.APP_PATH_ROUTES_MANIFEST), appPathRoutes);
            }
            process.env.NEXT_PHASE = _constants1.PHASE_PRODUCTION_BUILD;
            staticWorker = createStaticWorker(config, {
                numberOfWorkers,
                debuggerPortOffset: -1
            });
            const analysisBegin = process.hrtime();
            const staticCheckSpan = nextBuildSpan.traceChild('static-check');
            const functionsConfigManifest = {
                version: 1,
                functions: {}
            };
            const { customAppGetInitialProps, namedExports, isNextImageImported, hasNonStaticErrorPage } = await staticCheckSpan.traceAsyncFn(async ()=>{
                var _config_experimental_sri;
                if (isCompileMode) {
                    return {
                        customAppGetInitialProps: false,
                        namedExports: [],
                        isNextImageImported: true,
                        hasNonStaticErrorPage: hasUserPagesRoutes
                    };
                }
                const { configFileName } = config;
                const sriEnabled = Boolean((_config_experimental_sri = config.experimental.sri) == null ? void 0 : _config_experimental_sri.algorithm);
                const nonStaticErrorPageSpan = staticCheckSpan.traceChild('check-static-error-page');
                const errorPageHasCustomGetInitialProps = nonStaticErrorPageSpan.traceAsyncFn(async ()=>hasCustomErrorPage && await staticWorker.hasCustomGetInitialProps({
                        page: '/_error',
                        distDir,
                        checkingApp: false,
                        sriEnabled
                    }));
                const errorPageStaticResult = nonStaticErrorPageSpan.traceAsyncFn(async ()=>{
                    var _config_i18n, _config_i18n1;
                    return hasCustomErrorPage && staticWorker.isPageStatic({
                        dir,
                        page: '/_error',
                        distDir,
                        configFileName,
                        cacheComponents: isAppCacheComponentsEnabled,
                        authInterrupts: isAuthInterruptsEnabled,
                        httpAgentOptions: config.httpAgentOptions,
                        locales: (_config_i18n = config.i18n) == null ? void 0 : _config_i18n.locales,
                        defaultLocale: (_config_i18n1 = config.i18n) == null ? void 0 : _config_i18n1.defaultLocale,
                        nextConfigOutput: config.output,
                        pprConfig: config.experimental.ppr,
                        cacheLifeProfiles: config.cacheLife,
                        buildId,
                        sriEnabled,
                        cacheMaxMemorySize: config.cacheMaxMemorySize
                    });
                });
                const appPageToCheck = '/_app';
                const customAppGetInitialPropsPromise = hasUserPagesRoutes ? staticWorker.hasCustomGetInitialProps({
                    page: appPageToCheck,
                    distDir,
                    checkingApp: true,
                    sriEnabled
                }) : Promise.resolve(false);
                const namedExportsPromise = hasUserPagesRoutes ? staticWorker.getDefinedNamedExports({
                    page: appPageToCheck,
                    distDir,
                    sriEnabled
                }) : Promise.resolve([]);
                // eslint-disable-next-line @typescript-eslint/no-shadow
                let isNextImageImported;
                const middlewareManifest = require(_path.default.join(distDir, _constants1.SERVER_DIRECTORY, _constants1.MIDDLEWARE_MANIFEST));
                const actionManifest = appDir ? require(_path.default.join(distDir, _constants1.SERVER_DIRECTORY, _constants1.SERVER_REFERENCE_MANIFEST + '.json')) : null;
                const entriesWithAction = actionManifest ? new Set() : null;
                if (actionManifest && entriesWithAction) {
                    for(const id in actionManifest.node){
                        for(const entry in actionManifest.node[id].workers){
                            entriesWithAction.add(entry);
                        }
                    }
                    for(const id in actionManifest.edge){
                        for(const entry in actionManifest.edge[id].workers){
                            entriesWithAction.add(entry);
                        }
                    }
                }
                for (const key of Object.keys(middlewareManifest == null ? void 0 : middlewareManifest.functions)){
                    if (key.startsWith('/api')) {
                        edgeRuntimePagesCount++;
                    }
                }
                await Promise.all(Object.entries(pageKeys).reduce((acc, [key, files])=>{
                    if (!files) {
                        return acc;
                    }
                    const pageType = key;
                    for (const page of files){
                        acc.push({
                            pageType,
                            page
                        });
                    }
                    return acc;
                }, []).map(({ pageType, page })=>{
                    const checkPageSpan = staticCheckSpan.traceChild('check-page', {
                        page
                    });
                    return checkPageSpan.traceAsyncFn(async ()=>{
                        const actualPage = (0, _normalizepagepath.normalizePagePath)(page);
                        let isRoutePPREnabled = false;
                        let isSSG = false;
                        let isStatic = false;
                        let isServerComponent = false;
                        let ssgPageRoutes = null;
                        let pagePath = '';
                        if (pageType === 'pages') {
                            pagePath = pagesPaths.find((p)=>{
                                p = (0, _normalizepathsep.normalizePathSep)(p);
                                return p.startsWith(actualPage + '.') || p.startsWith(actualPage + '/index.');
                            }) || '';
                        }
                        let originalAppPath;
                        if (pageType === 'app' && mappedAppPages) {
                            for (const [originalPath, normalizedPath] of Object.entries(appPathRoutes)){
                                if (normalizedPath === page) {
                                    pagePath = mappedAppPages[originalPath].replace(/^private-next-app-dir/, '');
                                    originalAppPath = originalPath;
                                    break;
                                }
                            }
                        }
                        const pageFilePath = (0, _utils1.isAppBuiltinPage)(pagePath) ? pagePath : _path.default.join((pageType === 'pages' ? pagesDir : appDir) || '', pagePath);
                        const isInsideAppDir = pageType === 'app';
                        const staticInfo = pagePath ? await (0, _getstaticinfoincludinglayouts.getStaticInfoIncludingLayouts)({
                            isInsideAppDir,
                            pageFilePath,
                            pageExtensions: config.pageExtensions,
                            appDir,
                            config,
                            isDev: false,
                            // If this route is an App Router page route, inherit the
                            // route segment configs (e.g. `runtime`) from the layout by
                            // passing the `originalAppPath`, which should end with `/page`.
                            page: isInsideAppDir ? originalAppPath : page
                        }) : undefined;
                        if (staticInfo == null ? void 0 : staticInfo.hadUnsupportedValue) {
                            errorFromUnsupportedSegmentConfig();
                        }
                        // If there's any thing that would contribute to the functions
                        // configuration, we need to add it to the manifest.
                        if (typeof (staticInfo == null ? void 0 : staticInfo.runtime) !== 'undefined' || typeof (staticInfo == null ? void 0 : staticInfo.maxDuration) !== 'undefined' || typeof (staticInfo == null ? void 0 : staticInfo.preferredRegion) !== 'undefined') {
                            const regions = (staticInfo == null ? void 0 : staticInfo.preferredRegion) ? typeof staticInfo.preferredRegion === 'string' ? [
                                staticInfo.preferredRegion
                            ] : staticInfo.preferredRegion : undefined;
                            functionsConfigManifest.functions[page] = {
                                maxDuration: staticInfo == null ? void 0 : staticInfo.maxDuration,
                                ...regions && {
                                    regions
                                }
                            };
                        }
                        const pageRuntime = middlewareManifest.functions[originalAppPath || page] ? 'edge' : staticInfo == null ? void 0 : staticInfo.runtime;
                        if (!isCompileMode) {
                            isServerComponent = pageType === 'app' && (staticInfo == null ? void 0 : staticInfo.rsc) !== _constants1.RSC_MODULE_TYPES.client;
                            if (pageType === 'app' || !(0, _utils1.isReservedPage)(page)) {
                                try {
                                    let edgeInfo;
                                    if ((0, _isedgeruntime.isEdgeRuntime)(pageRuntime)) {
                                        if (pageType === 'app') {
                                            edgeRuntimeAppCount++;
                                        } else {
                                            edgeRuntimePagesCount++;
                                        }
                                        const manifestKey = pageType === 'pages' ? page : originalAppPath || '';
                                        edgeInfo = middlewareManifest.functions[manifestKey];
                                    }
                                    let isPageStaticSpan = checkPageSpan.traceChild('is-page-static');
                                    let workerResult = await isPageStaticSpan.traceAsyncFn(()=>{
                                        var _config_i18n, _config_i18n1;
                                        return staticWorker.isPageStatic({
                                            dir,
                                            page,
                                            originalAppPath,
                                            distDir,
                                            configFileName,
                                            httpAgentOptions: config.httpAgentOptions,
                                            locales: (_config_i18n = config.i18n) == null ? void 0 : _config_i18n.locales,
                                            defaultLocale: (_config_i18n1 = config.i18n) == null ? void 0 : _config_i18n1.defaultLocale,
                                            parentId: isPageStaticSpan.getId(),
                                            pageRuntime,
                                            edgeInfo,
                                            pageType,
                                            cacheComponents: isAppCacheComponentsEnabled,
                                            authInterrupts: isAuthInterruptsEnabled,
                                            cacheHandler: config.cacheHandler,
                                            cacheHandlers: config.cacheHandlers,
                                            isrFlushToDisk: _ciinfo.hasNextSupport ? false : config.experimental.isrFlushToDisk,
                                            cacheMaxMemorySize: config.cacheMaxMemorySize,
                                            nextConfigOutput: config.output,
                                            pprConfig: config.experimental.ppr,
                                            cacheLifeProfiles: config.cacheLife,
                                            buildId,
                                            sriEnabled
                                        });
                                    });
                                    if (pageType === 'app' && originalAppPath) {
                                        appNormalizedPaths.set(originalAppPath, page);
                                        // TODO-APP: handle prerendering with edge
                                        if ((0, _isedgeruntime.isEdgeRuntime)(pageRuntime)) {
                                            isStatic = false;
                                            isSSG = false;
                                            _log.warnOnce(`Using edge runtime on a page currently disables static generation for that page`);
                                        } else {
                                            const isDynamic = (0, _utils.isDynamicRoute)(page);
                                            if (typeof workerResult.isRoutePPREnabled === 'boolean') {
                                                isRoutePPREnabled = workerResult.isRoutePPREnabled;
                                            }
                                            // If this route can be partially pre-rendered, then
                                            // mark it as such and mark that it can be
                                            // generated server-side.
                                            if (workerResult.isRoutePPREnabled) {
                                                isSSG = true;
                                                isStatic = true;
                                                staticPaths.set(originalAppPath, []);
                                            }
                                            if (workerResult.prerenderedRoutes) {
                                                staticPaths.set(originalAppPath, workerResult.prerenderedRoutes);
                                                ssgPageRoutes = workerResult.prerenderedRoutes.map((route)=>route.pathname);
                                                isSSG = true;
                                            }
                                            const appConfig = workerResult.appConfig || {};
                                            if (appConfig.revalidate !== 0) {
                                                const hasGenerateStaticParams = workerResult.prerenderedRoutes && workerResult.prerenderedRoutes.length > 0;
                                                if (config.output === 'export' && isDynamic && !hasGenerateStaticParams) {
                                                    throw Object.defineProperty(new Error(`Page "${page}" is missing "generateStaticParams()" so it cannot be used with "output: export" config.`), "__NEXT_ERROR_CODE", {
                                                        value: "E87",
                                                        enumerable: false,
                                                        configurable: true
                                                    });
                                                }
                                                // Mark the app as static if:
                                                // - It has no dynamic param
                                                // - It doesn't have generateStaticParams but `dynamic` is set to
                                                //   `error` or `force-static`
                                                if (!isDynamic) {
                                                    staticPaths.set(originalAppPath, [
                                                        {
                                                            params: {},
                                                            pathname: page,
                                                            encodedPathname: page,
                                                            fallbackRouteParams: [],
                                                            fallbackMode: workerResult.prerenderFallbackMode,
                                                            fallbackRootParams: [],
                                                            throwOnEmptyStaticShell: true
                                                        }
                                                    ]);
                                                    isStatic = true;
                                                } else if (!hasGenerateStaticParams && (appConfig.dynamic === 'error' || appConfig.dynamic === 'force-static')) {
                                                    staticPaths.set(originalAppPath, []);
                                                    isStatic = true;
                                                    isRoutePPREnabled = false;
                                                }
                                            }
                                            if (workerResult.prerenderFallbackMode) {
                                                fallbackModes.set(originalAppPath, workerResult.prerenderFallbackMode);
                                            }
                                            appDefaultConfigs.set(originalAppPath, appConfig);
                                        }
                                    } else {
                                        if ((0, _isedgeruntime.isEdgeRuntime)(pageRuntime)) {
                                            if (workerResult.hasStaticProps) {
                                                console.warn(`"getStaticProps" is not yet supported fully with "experimental-edge", detected on ${page}`);
                                            }
                                            workerResult.isStatic = false;
                                            workerResult.hasStaticProps = false;
                                        }
                                        if (workerResult.isNextImageImported) {
                                            isNextImageImported = true;
                                        }
                                        if (workerResult.hasStaticProps) {
                                            ssgPages.add(page);
                                            isSSG = true;
                                            if (workerResult.prerenderedRoutes && workerResult.prerenderedRoutes.length > 0) {
                                                additionalPaths.set(page, workerResult.prerenderedRoutes);
                                                ssgPageRoutes = workerResult.prerenderedRoutes.map((route)=>route.pathname);
                                            }
                                            if (workerResult.prerenderFallbackMode === _fallback.FallbackMode.BLOCKING_STATIC_RENDER) {
                                                ssgBlockingFallbackPages.add(page);
                                            } else if (workerResult.prerenderFallbackMode === _fallback.FallbackMode.PRERENDER) {
                                                ssgStaticFallbackPages.add(page);
                                            }
                                        } else if (workerResult.hasServerProps) {
                                            serverPropsPages.add(page);
                                        } else if (workerResult.isStatic && !isServerComponent && await customAppGetInitialPropsPromise === false) {
                                            staticPages.add(page);
                                            isStatic = true;
                                        } else if (isServerComponent) {
                                            // This is a static server component page that doesn't have
                                            // gSP or gSSP. We still treat it as a SSG page.
                                            ssgPages.add(page);
                                            isSSG = true;
                                        }
                                        if (hasPages404 && page === '/404') {
                                            if (!workerResult.isStatic && !workerResult.hasStaticProps) {
                                                throw Object.defineProperty(new Error(`\`pages/404\` ${_constants.STATIC_STATUS_PAGE_GET_INITIAL_PROPS_ERROR}`), "__NEXT_ERROR_CODE", {
                                                    value: "E134",
                                                    enumerable: false,
                                                    configurable: true
                                                });
                                            }
                                            // we need to ensure the 404 lambda is present since we use
                                            // it when _app has getInitialProps
                                            if (await customAppGetInitialPropsPromise && !workerResult.hasStaticProps) {
                                                staticPages.delete(page);
                                            }
                                        }
                                        if (_constants1.STATIC_STATUS_PAGES.includes(page) && !workerResult.isStatic && !workerResult.hasStaticProps) {
                                            throw Object.defineProperty(new Error(`\`pages${page}\` ${_constants.STATIC_STATUS_PAGE_GET_INITIAL_PROPS_ERROR}`), "__NEXT_ERROR_CODE", {
                                                value: "E125",
                                                enumerable: false,
                                                configurable: true
                                            });
                                        }
                                    }
                                } catch (err) {
                                    if (!(0, _iserror.default)(err) || err.message !== 'INVALID_DEFAULT_EXPORT') throw err;
                                    invalidPages.add(page);
                                }
                            }
                            if (pageType === 'app') {
                                if (isSSG || isStatic) {
                                    staticAppPagesCount++;
                                } else {
                                    serverAppPagesCount++;
                                }
                            }
                        }
                        pageInfos.set(page, {
                            originalAppPath,
                            isStatic,
                            isSSG,
                            isRoutePPREnabled,
                            ssgPageRoutes,
                            initialCacheControl: undefined,
                            runtime: pageRuntime,
                            pageDuration: undefined,
                            ssgPageDurations: undefined,
                            hasEmptyStaticShell: undefined
                        });
                    });
                }));
                const errorPageResult = await errorPageStaticResult;
                const nonStaticErrorPage = await errorPageHasCustomGetInitialProps || errorPageResult && errorPageResult.hasServerProps;
                const returnValue = {
                    customAppGetInitialProps: await customAppGetInitialPropsPromise,
                    namedExports: await namedExportsPromise,
                    isNextImageImported,
                    hasNonStaticErrorPage: nonStaticErrorPage
                };
                return returnValue;
            });
            if (postCompileSpinner) {
                const collectingPageDataEnd = process.hrtime(collectingPageDataStart);
                postCompileSpinner.setText(`Collecting page data using ${numberOfWorkers} worker${numberOfWorkers > 1 ? 's' : ''} in ${(0, _durationtostring.hrtimeDurationToString)(collectingPageDataEnd)}`);
                postCompileSpinner.stopAndPersist();
            }
            (0, _trace1.traceMemoryUsage)('Finished collecting page data', nextBuildSpan);
            if (customAppGetInitialProps) {
                console.warn((0, _picocolors.bold)((0, _picocolors.yellow)(`Warning: `)) + (0, _picocolors.yellow)(`You have opted-out of Automatic Static Optimization due to \`getInitialProps\` in \`pages/_app\`. This does not opt-out pages with \`getStaticProps\``));
                console.warn('Read more: https://nextjs.org/docs/messages/opt-out-auto-static-optimization\n');
            }
            const middlewareFile = (0, _normalizepathsep.normalizePathSep)(proxyFilePath || middlewareFilePath || '');
            let hasNodeMiddleware = false;
            if (middlewareFile) {
                // Is format of `(/src)/(proxy|middleware).<ext>`, so split by
                // "." and get the first part, regard rest of the extensions
                // to match the `page` value format.
                const page = middlewareFile.split('.')[0];
                const staticInfo = await (0, _getstaticinfoincludinglayouts.getStaticInfoIncludingLayouts)({
                    isInsideAppDir: false,
                    pageFilePath: _path.default.join(dir, middlewareFile),
                    config,
                    appDir,
                    pageExtensions: config.pageExtensions,
                    isDev: false,
                    page
                });
                if (staticInfo.hadUnsupportedValue) {
                    errorFromUnsupportedSegmentConfig();
                }
                if (staticInfo.runtime === 'nodejs' || (0, _utils1.isProxyFile)(page)) {
                    var _staticInfo_middleware;
                    hasNodeMiddleware = true;
                    functionsConfigManifest.functions['/_middleware'] = {
                        runtime: 'nodejs',
                        matchers: ((_staticInfo_middleware = staticInfo.middleware) == null ? void 0 : _staticInfo_middleware.matchers) ?? [
                            {
                                regexp: '^.*$',
                                originalSource: '/:path*'
                            }
                        ]
                    };
                    if (bundler === _bundler.Bundler.Turbopack) {
                        await writeManifest(_path.default.join(distDir, 'static', buildId, _constants1.TURBOPACK_CLIENT_MIDDLEWARE_MANIFEST), functionsConfigManifest.functions['/_middleware'].matchers || []);
                    }
                }
            }
            await writeFunctionsConfigManifest(distDir, functionsConfigManifest);
            if (bundler !== _bundler.Bundler.Turbopack && !isGenerateMode && !buildTracesPromise) {
                buildTracesPromise = nextBuildSpan.traceChild('collect-build-traces').traceAsyncFn(()=>{
                    return (0, _collectbuildtraces.collectBuildTraces)({
                        dir,
                        config,
                        distDir,
                        edgeRuntimeRoutes: (0, _utils1.collectRoutesUsingEdgeRuntime)(pageInfos),
                        staticPages: [
                            ...staticPages
                        ],
                        nextBuildSpan,
                        buildTraceContext,
                        outputFileTracingRoot
                    }).catch((err)=>{
                        console.error(err);
                        throw err;
                    });
                });
            }
            if (serverPropsPages.size > 0 || ssgPages.size > 0) {
                // We update the routes manifest after the build with the
                // data routes since we can't determine these until after build
                routesManifest.dataRoutes = (0, _sortableroutes.sortPages)([
                    ...serverPropsPages,
                    ...ssgPages
                ]).map((page)=>{
                    return (0, _builddataroute.buildDataRoute)(page, buildId);
                });
            }
            // We need to write the manifest with rewrites before build
            await nextBuildSpan.traceChild('write-routes-manifest').traceAsyncFn(()=>writeManifest(routesManifestPath, routesManifest));
            // Since custom _app.js can wrap the 404 page we have to opt-out of static optimization if it has getInitialProps
            // Only export the static 404 when there is no /_error present
            const useStaticPages404 = !customAppGetInitialProps && (!hasNonStaticErrorPage || hasPages404);
            if (invalidPages.size > 0) {
                const err = Object.defineProperty(new Error(`Build optimization failed: found page${invalidPages.size === 1 ? '' : 's'} without a React Component as default export in \n${[
                    ...invalidPages
                ].map((pg)=>`pages${pg}`).join('\n')}\n\nSee https://nextjs.org/docs/messages/page-without-valid-component for more info.\n`), "__NEXT_ERROR_CODE", {
                    value: "E474",
                    enumerable: false,
                    configurable: true
                });
                err.code = 'BUILD_OPTIMIZATION_FAILED';
                throw err;
            }
            await (0, _writebuildid.writeBuildId)(distDir, buildId);
            const features = [
                {
                    featureName: 'experimental/cacheComponents',
                    invocationCount: config.cacheComponents ? 1 : 0
                },
                {
                    featureName: 'experimental/optimizeCss',
                    invocationCount: config.experimental.optimizeCss ? 1 : 0
                },
                {
                    featureName: 'experimental/nextScriptWorkers',
                    invocationCount: config.experimental.nextScriptWorkers ? 1 : 0
                },
                {
                    featureName: 'experimental/ppr',
                    invocationCount: config.experimental.ppr ? 1 : 0
                },
                {
                    featureName: 'experimental/isolatedDevBuild',
                    invocationCount: config.experimental.isolatedDevBuild ? 1 : 0
                },
                {
                    featureName: 'turbopackFileSystemCache',
                    invocationCount: (0, _utils3.isFileSystemCacheEnabledForBuild)(config) ? 1 : 0
                }
            ];
            telemetry.record(features.map((feature)=>{
                return {
                    eventName: _events.EVENT_BUILD_FEATURE_USAGE,
                    payload: feature
                };
            }));
            // we don't need to inline for turbopack build as
            // it will handle it's own caching separate of compile
            if (isGenerateMode && bundler !== _bundler.Bundler.Turbopack) {
                _log.info('Inlining static env ...');
                await nextBuildSpan.traceChild('inline-static-env').traceAsyncFn(async ()=>{
                    await (0, _inlinestaticenv.inlineStaticEnv)({
                        distDir,
                        config
                    });
                });
            }
            const middlewareManifest = await readManifest(_path.default.join(distDir, _constants1.SERVER_DIRECTORY, _constants1.MIDDLEWARE_MANIFEST));
            const prerenderManifest = {
                version: 4,
                routes: {},
                dynamicRoutes: {},
                notFoundRoutes: [],
                preview: previewProps
            };
            const tbdPrerenderRoutes = [];
            const { i18n } = config;
            const usedStaticStatusPages = _constants1.STATIC_STATUS_PAGES.filter((page)=>mappedPages[page] && mappedPages[page].startsWith('private-next-pages'));
            usedStaticStatusPages.forEach((page)=>{
                if (!ssgPages.has(page) && !customAppGetInitialProps) {
                    staticPages.add(page);
                }
            });
            const hasPages500 = !appDirOnly && usedStaticStatusPages.includes('/500');
            const useDefaultStatic500 = !hasPages500 && !hasNonStaticErrorPage && !customAppGetInitialProps;
            const combinedPages = [
                ...staticPages,
                ...ssgPages
            ];
            const isApp404Static = staticPaths.has(_entryconstants.UNDERSCORE_NOT_FOUND_ROUTE_ENTRY);
            const hasStaticApp404 = hasApp404 && isApp404Static;
            const isAppGlobalErrorStatic = staticPaths.has(_entryconstants.UNDERSCORE_GLOBAL_ERROR_ROUTE_ENTRY);
            const hasStaticAppGlobalError = hasAppGlobalError && isAppGlobalErrorStatic;
            await (0, _builddiagnostics.updateBuildDiagnostics)({
                buildStage: 'static-generation'
            });
            const hasGSPAndRevalidateZero = new Set();
            // we need to trigger automatic exporting when we have
            // - static 404/500
            // - getStaticProps paths
            // - experimental app is enabled
            if (!isCompileMode && (combinedPages.length > 0 || useStaticPages404 || useDefaultStatic500 || appDir)) {
                const staticGenerationSpan = nextBuildSpan.traceChild('static-generation');
                await staticGenerationSpan.traceAsyncFn(async ()=>{
                    (0, _utils1.detectConflictingPaths)([
                        ...combinedPages,
                        ...pageKeys.pages.filter((page)=>!combinedPages.includes(page))
                    ], ssgPages, new Map(Array.from(additionalPaths.entries()).map(([page, routes])=>{
                        return [
                            page,
                            routes.map((route)=>route.pathname)
                        ];
                    })));
                    const sortedStaticPaths = Array.from(staticPaths.entries()).sort(([a], [b])=>a.localeCompare(b));
                    const exportApp = require('../export').default;
                    const exportConfig = {
                        ...config,
                        // Default map will be the collection of automatic statically exported
                        // pages and incremental pages.
                        // n.b. we cannot handle this above in combinedPages because the dynamic
                        // page must be in the `pages` array, but not in the mapping.
                        exportPathMap: (defaultMap)=>{
                            // Dynamically routed pages should be prerendered to be used as
                            // a client-side skeleton (fallback) while data is being fetched.
                            // This ensures the end-user never sees a 500 or slow response from the
                            // server.
                            //
                            // Note: prerendering disables automatic static optimization.
                            ssgPages.forEach((page)=>{
                                if ((0, _utils.isDynamicRoute)(page)) {
                                    tbdPrerenderRoutes.push(page);
                                    if (ssgStaticFallbackPages.has(page)) {
                                        // Override the rendering for the dynamic page to be treated as a
                                        // fallback render.
                                        if (i18n) {
                                            defaultMap[`/${i18n.defaultLocale}${page}`] = {
                                                page,
                                                _pagesFallback: true
                                            };
                                        } else {
                                            defaultMap[page] = {
                                                page,
                                                _pagesFallback: true
                                            };
                                        }
                                    } else {
                                        // Remove dynamically routed pages from the default path map when
                                        // fallback behavior is disabled.
                                        delete defaultMap[page];
                                    }
                                }
                            });
                            // Append the "well-known" routes we should prerender for, e.g. blog
                            // post slugs.
                            additionalPaths.forEach((routes, page)=>{
                                routes.forEach((route)=>{
                                    defaultMap[route.pathname] = {
                                        page,
                                        _ssgPath: route.encodedPathname
                                    };
                                });
                            });
                            if (useStaticPages404 && !appDirOnly) {
                                defaultMap['/404'] = {
                                    page: hasPages404 ? '/404' : '/_error'
                                };
                            }
                            if (useDefaultStatic500 && !appDirOnly) {
                                defaultMap['/500'] = {
                                    page: '/_error'
                                };
                            }
                            // TODO: output manifest specific to app paths and their
                            // revalidate periods and dynamicParams settings
                            sortedStaticPaths.forEach(([originalAppPath, routes])=>{
                                const appConfig = appDefaultConfigs.get(originalAppPath);
                                const isDynamicError = (appConfig == null ? void 0 : appConfig.dynamic) === 'error';
                                const isRoutePPREnabled = appConfig ? (0, _ppr.checkIsRoutePPREnabled)(config.experimental.ppr) : false;
                                routes.forEach((route)=>{
                                    // If the route has any dynamic root segments, we need to skip
                                    // rendering the route. This is because we don't support
                                    // revalidating the shells without the parameters present.
                                    // Note that we only have fallback root params if we also have
                                    // PPR enabled for this route/app already.
                                    if (route.fallbackRootParams && route.fallbackRootParams.length > 0 && // We don't skip rendering the route if we have the
                                    // following enabled. This is because the flight data now
                                    // does not contain any of the route params and is instead
                                    // completely static.
                                    !config.cacheComponents) {
                                        return;
                                    }
                                    defaultMap[route.pathname] = {
                                        page: originalAppPath,
                                        _ssgPath: route.encodedPathname,
                                        _fallbackRouteParams: route.fallbackRouteParams,
                                        _isDynamicError: isDynamicError,
                                        _isAppDir: true,
                                        _isRoutePPREnabled: isRoutePPREnabled,
                                        _allowEmptyStaticShell: !route.throwOnEmptyStaticShell
                                    };
                                });
                            });
                            if (i18n) {
                                for (const page of [
                                    ...staticPages,
                                    ...ssgPages,
                                    ...useStaticPages404 ? [
                                        '/404'
                                    ] : [],
                                    ...useDefaultStatic500 ? [
                                        '/500'
                                    ] : []
                                ]){
                                    const isSsg = ssgPages.has(page);
                                    const isDynamic = (0, _utils.isDynamicRoute)(page);
                                    const isFallback = isSsg && ssgStaticFallbackPages.has(page);
                                    for (const locale of i18n.locales){
                                        var _defaultMap_page;
                                        // skip fallback generation for SSG pages without fallback mode
                                        if (isSsg && isDynamic && !isFallback) continue;
                                        const outputPath = `/${locale}${page === '/' ? '' : page}`;
                                        defaultMap[outputPath] = {
                                            page: ((_defaultMap_page = defaultMap[page]) == null ? void 0 : _defaultMap_page.page) || page,
                                            _locale: locale,
                                            _pagesFallback: isFallback
                                        };
                                    }
                                    if (isSsg) {
                                        // remove non-locale prefixed variant from defaultMap
                                        delete defaultMap[page];
                                    }
                                }
                            }
                            return defaultMap;
                        }
                    };
                    const outdir = _path.default.join(distDir, 'export');
                    const exportResult = await exportApp(dir, {
                        nextConfig: exportConfig,
                        enabledDirectories,
                        silent: true,
                        buildExport: true,
                        debugOutput,
                        debugPrerender,
                        pages: combinedPages,
                        outdir,
                        statusMessage: `Generating static pages using ${numberOfWorkers} worker${numberOfWorkers > 1 ? 's' : ''}`,
                        numWorkers: numberOfWorkers,
                        appDirOnly
                    }, nextBuildSpan, staticWorker);
                    // If there was no result, there's nothing more to do.
                    if (!exportResult) return;
                    const getFallbackMode = (route)=>{
                        var _exportResult_byPath_get;
                        const hasEmptyStaticShell = (_exportResult_byPath_get = exportResult.byPath.get(route.pathname)) == null ? void 0 : _exportResult_byPath_get.hasEmptyStaticShell;
                        // If the route has an empty static shell and is not configured to
                        // throw on empty static shell, then we should use the blocking
                        // static render mode.
                        if (hasEmptyStaticShell && !route.throwOnEmptyStaticShell && route.fallbackMode === _fallback.FallbackMode.PRERENDER) {
                            return _fallback.FallbackMode.BLOCKING_STATIC_RENDER;
                        }
                        // If the route has no fallback mode, then we should use the
                        // `NOT_FOUND` fallback mode.
                        if (!route.fallbackMode) {
                            return _fallback.FallbackMode.NOT_FOUND;
                        }
                        return route.fallbackMode;
                    };
                    const getCacheControl = (exportPath, defaultRevalidate = false)=>{
                        var _exportResult_byPath_get;
                        const cacheControl = (_exportResult_byPath_get = exportResult.byPath.get(exportPath)) == null ? void 0 : _exportResult_byPath_get.cacheControl;
                        if (!cacheControl) {
                            return {
                                revalidate: defaultRevalidate,
                                expire: undefined
                            };
                        }
                        if (cacheControl.revalidate !== false && cacheControl.revalidate > 0 && cacheControl.expire === undefined) {
                            return {
                                revalidate: cacheControl.revalidate,
                                expire: config.expireTime
                            };
                        }
                        return cacheControl;
                    };
                    if (debugOutput || process.env.NEXT_SSG_FETCH_METRICS === '1') {
                        (0, _builddiagnostics.recordFetchMetrics)(exportResult);
                    }
                    (0, _turborepoaccesstrace.writeTurborepoAccessTraceResult)({
                        distDir: config.distDir,
                        traces: [
                            turborepoAccessTraceResult,
                            ...exportResult.turborepoAccessTraceResults.values()
                        ]
                    });
                    prerenderManifest.notFoundRoutes = Array.from(exportResult.ssgNotFoundPaths);
                    // remove server bundles that were exported
                    for (const page of staticPages){
                        const serverBundle = (0, _require.getPagePath)(page, distDir, undefined, false);
                        await _fs.promises.unlink(serverBundle);
                    }
                    sortedStaticPaths.forEach(([originalAppPath, prerenderedRoutes])=>{
                        var _pageInfos_get, _pageInfos_get1;
                        const page = appNormalizedPaths.get(originalAppPath);
                        if (!page) throw Object.defineProperty(new _invarianterror.InvariantError('Page not found'), "__NEXT_ERROR_CODE", {
                            value: "E619",
                            enumerable: false,
                            configurable: true
                        });
                        const appConfig = appDefaultConfigs.get(originalAppPath);
                        if (!appConfig) throw Object.defineProperty(new _invarianterror.InvariantError('App config not found'), "__NEXT_ERROR_CODE", {
                            value: "E616",
                            enumerable: false,
                            configurable: true
                        });
                        const ssgPageRoutesSet = new Set((_pageInfos_get = pageInfos.get(page)) == null ? void 0 : _pageInfos_get.ssgPageRoutes);
                        let hasRevalidateZero = appConfig.revalidate === 0 || getCacheControl(page).revalidate === 0;
                        if (hasRevalidateZero && ((_pageInfos_get1 = pageInfos.get(page)) == null ? void 0 : _pageInfos_get1.isStatic)) {
                            // if the page was marked as being static, but it contains dynamic data
                            // (ie, in the case of a static generation bailout), then it should be marked dynamic
                            pageInfos.set(page, {
                                ...pageInfos.get(page),
                                isStatic: false,
                                isSSG: false
                            });
                        }
                        const isAppRouteHandler = (0, _isapprouteroute.isAppRouteRoute)(originalAppPath);
                        // When this is an app page and PPR is enabled, the route supports
                        // partial pre-rendering.
                        const isRoutePPREnabled = !isAppRouteHandler && (0, _ppr.checkIsRoutePPREnabled)(config.experimental.ppr) ? true : undefined;
                        const htmlBotsRegexString = // The htmlLimitedBots has been converted to a string during loadConfig
                        config.htmlLimitedBots || _isbot.HTML_LIMITED_BOT_UA_RE_STRING;
                        // this flag is used to selectively bypass the static cache and invoke the lambda directly
                        // to enable server actions on static routes
                        const bypassFor = [
                            {
                                type: 'header',
                                key: _approuterheaders.ACTION_HEADER
                            },
                            {
                                type: 'header',
                                key: 'content-type',
                                value: 'multipart/form-data;.*'
                            },
                            // If it's PPR rendered non-static page, bypass the PPR cache when streaming metadata is enabled.
                            // This will skip the postpone data for those bots requests and instead produce a dynamic render.
                            ...isRoutePPREnabled ? [
                                {
                                    type: 'header',
                                    key: 'user-agent',
                                    value: htmlBotsRegexString
                                }
                            ] : []
                        ];
                        // We should collect all the dynamic routes into a single array for
                        // this page. Including the full fallback route (the original
                        // route), any routes that were generated with unknown route params
                        // should be collected and included in the dynamic routes part
                        // of the manifest instead.
                        const staticPrerenderedRoutes = [];
                        const dynamicPrerenderedRoutes = [];
                        // Sort the outputted routes to ensure consistent output. Any route
                        // though that has unknown route params will be pulled and sorted
                        // independently. This is because the routes with unknown route
                        // params will contain the dynamic path parameters, some of which
                        // may conflict with the actual prerendered routes.
                        const unsortedUnknownPrerenderRoutes = [];
                        const unsortedKnownPrerenderRoutes = [];
                        for (const prerenderedRoute of prerenderedRoutes){
                            if (prerenderedRoute.fallbackRouteParams && prerenderedRoute.fallbackRouteParams.length > 0) {
                                unsortedUnknownPrerenderRoutes.push(prerenderedRoute);
                            } else {
                                unsortedKnownPrerenderRoutes.push(prerenderedRoute);
                            }
                        }
                        const sortedUnknownPrerenderRoutes = (0, _sortableroutes.sortPageObjects)(unsortedUnknownPrerenderRoutes, (prerenderedRoute)=>prerenderedRoute.pathname);
                        const sortedKnownPrerenderRoutes = (0, _sortableroutes.sortPageObjects)(unsortedKnownPrerenderRoutes, (prerenderedRoute)=>prerenderedRoute.pathname);
                        prerenderedRoutes = [
                            ...sortedKnownPrerenderRoutes,
                            ...sortedUnknownPrerenderRoutes
                        ];
                        for (const prerenderedRoute of prerenderedRoutes){
                            if (isRoutePPREnabled && prerenderedRoute.fallbackRouteParams && prerenderedRoute.fallbackRouteParams.length > 0) {
                                // If the route has unknown params, then we need to add it to
                                // the list of dynamic routes.
                                dynamicPrerenderedRoutes.push(prerenderedRoute);
                            } else {
                                // If the route doesn't have unknown params, then we need to
                                // add it to the list of static routes.
                                staticPrerenderedRoutes.push(prerenderedRoute);
                            }
                        }
                        // Handle all the static routes.
                        for (const route of staticPrerenderedRoutes){
                            if ((0, _utils.isDynamicRoute)(page) && route.pathname === page) continue;
                            const { metadata = {}, hasEmptyStaticShell, hasPostponed } = exportResult.byPath.get(route.pathname) ?? {};
                            const cacheControl = getCacheControl(route.pathname, appConfig.revalidate);
                            pageInfos.set(route.pathname, {
                                ...pageInfos.get(route.pathname),
                                hasPostponed,
                                hasEmptyStaticShell,
                                initialCacheControl: cacheControl
                            });
                            // update the page (eg /blog/[slug]) to also have the postpone metadata
                            pageInfos.set(page, {
                                ...pageInfos.get(page),
                                hasPostponed,
                                hasEmptyStaticShell,
                                initialCacheControl: cacheControl
                            });
                            if (cacheControl.revalidate !== 0) {
                                const normalizedRoute = (0, _normalizepagepath.normalizePagePath)(route.pathname);
                                let dataRoute;
                                if (isAppRouteHandler) {
                                    dataRoute = null;
                                } else {
                                    dataRoute = _path.default.posix.join(`${normalizedRoute}${_constants.RSC_SUFFIX}`);
                                }
                                const meta = (0, _utils1.collectMeta)(metadata);
                                const status = route.pathname === _entryconstants.UNDERSCORE_NOT_FOUND_ROUTE ? 404 : meta.status;
                                prerenderManifest.routes[route.pathname] = {
                                    initialStatus: status,
                                    initialHeaders: meta.headers,
                                    renderingMode: isAppPPREnabled ? isRoutePPREnabled ? _renderingmode.RenderingMode.PARTIALLY_STATIC : _renderingmode.RenderingMode.STATIC : undefined,
                                    experimentalPPR: isRoutePPREnabled,
                                    experimentalBypassFor: bypassFor,
                                    initialRevalidateSeconds: cacheControl.revalidate,
                                    initialExpireSeconds: cacheControl.expire,
                                    srcRoute: page,
                                    dataRoute,
                                    prefetchDataRoute: undefined,
                                    allowHeader: ALLOWED_HEADERS
                                };
                            } else {
                                hasRevalidateZero = true;
                                if (ssgPageRoutesSet.has(route.pathname)) {
                                    const pageInfo = pageInfos.get(page);
                                    // Remove the route from the SSG page routes if it bailed out
                                    // during prerendering.
                                    ssgPageRoutesSet.delete(route.pathname);
                                    // Mark the route as having a GSP and revalidate zero.
                                    if (ssgPageRoutesSet.size === 0) {
                                        hasGSPAndRevalidateZero.delete(page);
                                    } else {
                                        hasGSPAndRevalidateZero.add(page);
                                    }
                                    pageInfos.set(page, {
                                        ...pageInfo,
                                        ssgPageRoutes: Array.from(ssgPageRoutesSet),
                                        // If there are no SSG page routes left, then the page is not SSG.
                                        isSSG: ssgPageRoutesSet.size === 0 ? false : pageInfo.isSSG
                                    });
                                } else {
                                    // we might have determined during prerendering that this page
                                    // used dynamic data
                                    pageInfos.set(route.pathname, {
                                        ...pageInfos.get(route.pathname),
                                        isSSG: false,
                                        isStatic: false
                                    });
                                }
                            }
                        }
                        if (!hasRevalidateZero && (0, _utils.isDynamicRoute)(page)) {
                            // When PPR fallbacks aren't used, we need to include it here. If
                            // they are enabled, then it'll already be included in the
                            // prerendered routes.
                            if (!isRoutePPREnabled) {
                                dynamicPrerenderedRoutes.push({
                                    params: {},
                                    pathname: page,
                                    encodedPathname: page,
                                    fallbackRouteParams: [],
                                    fallbackMode: fallbackModes.get(originalAppPath) ?? _fallback.FallbackMode.NOT_FOUND,
                                    fallbackRootParams: [],
                                    throwOnEmptyStaticShell: true
                                });
                            }
                            for (const route of dynamicPrerenderedRoutes){
                                var _exportResult_byPath_get;
                                const normalizedRoute = (0, _normalizepagepath.normalizePagePath)(route.pathname);
                                const metadata = (_exportResult_byPath_get = exportResult.byPath.get(route.pathname)) == null ? void 0 : _exportResult_byPath_get.metadata;
                                const cacheControl = getCacheControl(route.pathname);
                                let dataRoute = null;
                                if (!isAppRouteHandler) {
                                    dataRoute = _path.default.posix.join(`${normalizedRoute}${_constants.RSC_SUFFIX}`);
                                }
                                let prefetchDataRoute = null;
                                let dynamicRoute = routesManifest.dynamicRoutes.find((r)=>r.page === route.pathname);
                                if (!isAppRouteHandler && isAppPPREnabled) {
                                    // If the dynamic route wasn't found, then we need to create
                                    // it. This ensures that for each fallback shell there's an
                                    // entry in the app routes manifest which enables routing for
                                    // this fallback shell.
                                    if (!dynamicRoute) {
                                        dynamicRoute = (0, _utils1.pageToRoute)(route.pathname, page);
                                        sourcePages.set(route.pathname, page);
                                        // This route is not for the internal router, but instead
                                        // for external routers.
                                        dynamicRoute.skipInternalRouting = true;
                                        // Push this to the end of the array. The dynamic routes are
                                        // sorted by page later.
                                        dynamicRoutes.push(dynamicRoute);
                                    }
                                }
                                if (!isAppRouteHandler && ((metadata == null ? void 0 : metadata.segmentPaths) || route.fallbackRootParams && route.fallbackRootParams.length > 0)) {
                                    // If PPR isn't enabled, then we might not find the dynamic
                                    // route by pathname. If that's the case, we need to find the
                                    // route by page.
                                    if (!dynamicRoute) {
                                        dynamicRoute = dynamicRoutes.find((r)=>r.page === page);
                                        // If it can't be found by page, we must throw an error.
                                        if (!dynamicRoute) {
                                            throw Object.defineProperty(new _invarianterror.InvariantError('Dynamic route not found'), "__NEXT_ERROR_CODE", {
                                                value: "E633",
                                                enumerable: false,
                                                configurable: true
                                            });
                                        }
                                    }
                                    if (metadata == null ? void 0 : metadata.segmentPaths) {
                                        const pageSegmentPath = metadata.segmentPaths.find((item)=>item.endsWith('__PAGE__'));
                                        if (!pageSegmentPath) {
                                            throw Object.defineProperty(new Error(`Invariant: missing __PAGE__ segmentPath`), "__NEXT_ERROR_CODE", {
                                                value: "E759",
                                                enumerable: false,
                                                configurable: true
                                            });
                                        }
                                        // We build a combined segment data route from the
                                        // page segment as we need to limit the number of
                                        // routes we output and they can be shared
                                        const builtSegmentDataRoute = (0, _buildprefetchsegmentdataroute.buildPrefetchSegmentDataRoute)(route.pathname, pageSegmentPath);
                                        builtSegmentDataRoute.source = builtSegmentDataRoute.source.replace('/__PAGE__\\.segment\\.rsc$', `(?<segment>/__PAGE__\\.segment\\.rsc|\\.segment\\.rsc)(?:/)?$`);
                                        builtSegmentDataRoute.destination = builtSegmentDataRoute.destination.replace('/__PAGE__.segment.rsc', '$segment');
                                        dynamicRoute.prefetchSegmentDataRoutes ??= [];
                                        dynamicRoute.prefetchSegmentDataRoutes.push(builtSegmentDataRoute);
                                    }
                                }
                                pageInfos.set(route.pathname, {
                                    ...pageInfos.get(route.pathname),
                                    isDynamicAppRoute: true,
                                    // if PPR is turned on and the route contains a dynamic segment,
                                    // we assume it'll be partially prerendered
                                    hasPostponed: isRoutePPREnabled
                                });
                                const fallbackMode = getFallbackMode(route);
                                // When the route is configured to serve a prerender, we should
                                // use the cache control from the export result. If it can't be
                                // found, mark that we should keep the shell forever
                                // (revalidate: `false` via `getCacheControl()`).
                                const fallbackCacheControl = isRoutePPREnabled && fallbackMode === _fallback.FallbackMode.PRERENDER ? cacheControl : undefined;
                                const fallback = (0, _fallback.fallbackModeToFallbackField)(fallbackMode, route.pathname);
                                const meta = metadata && isRoutePPREnabled && fallbackMode === _fallback.FallbackMode.PRERENDER ? (0, _utils1.collectMeta)(metadata) : {};
                                prerenderManifest.dynamicRoutes[route.pathname] = {
                                    experimentalPPR: isRoutePPREnabled,
                                    renderingMode: isAppPPREnabled ? isRoutePPREnabled ? _renderingmode.RenderingMode.PARTIALLY_STATIC : _renderingmode.RenderingMode.STATIC : undefined,
                                    experimentalBypassFor: bypassFor,
                                    routeRegex: (0, _loadcustomroutes.normalizeRouteRegex)((0, _routeregex.getNamedRouteRegex)(route.pathname, {
                                        prefixRouteKeys: false
                                    }).re.source),
                                    dataRoute,
                                    fallback,
                                    fallbackRevalidate: fallbackCacheControl == null ? void 0 : fallbackCacheControl.revalidate,
                                    fallbackExpire: fallbackCacheControl == null ? void 0 : fallbackCacheControl.expire,
                                    fallbackStatus: meta.status,
                                    fallbackHeaders: meta.headers,
                                    fallbackRootParams: fallback ? route.fallbackRootParams : undefined,
                                    fallbackSourceRoute: route.fallbackRouteParams && route.fallbackRouteParams.length > 0 ? page : undefined,
                                    fallbackRouteParams: route.fallbackRouteParams,
                                    dataRouteRegex: !dataRoute ? null : (0, _loadcustomroutes.normalizeRouteRegex)((0, _routeregex.getNamedRouteRegex)(dataRoute, {
                                        prefixRouteKeys: false,
                                        includeSuffix: true,
                                        excludeOptionalTrailingSlash: true
                                    }).re.source),
                                    prefetchDataRoute,
                                    prefetchDataRouteRegex: !prefetchDataRoute ? undefined : (0, _loadcustomroutes.normalizeRouteRegex)((0, _routeregex.getNamedRouteRegex)(prefetchDataRoute, {
                                        prefixRouteKeys: false,
                                        includeSuffix: true,
                                        excludeOptionalTrailingSlash: true
                                    }).re.source),
                                    allowHeader: ALLOWED_HEADERS
                                };
                            }
                        }
                    });
                    const moveExportedPage = async (originPage, page, file, isSsg, ext, additionalSsgFile = false)=>{
                        return staticGenerationSpan.traceChild('move-exported-page').traceAsyncFn(async ()=>{
                            file = `${file}.${ext}`;
                            const orig = _path.default.join(outdir, file);
                            const pagePath = (0, _require.getPagePath)(originPage, distDir, undefined, false);
                            const relativeDest = _path.default.relative(_path.default.join(distDir, _constants1.SERVER_DIRECTORY), _path.default.join(_path.default.join(pagePath, // strip leading / and then recurse number of nested dirs
                            // to place from base folder
                            originPage.slice(1).split('/').map(()=>'..').join('/')), file)).replace(/\\/g, '/');
                            if (!isSsg && !// don't add static status page to manifest if it's
                            // the default generated version e.g. no pages/500
                            (_constants1.STATIC_STATUS_PAGES.includes(page) && !usedStaticStatusPages.includes(page))) {
                                pagesManifest[page] = relativeDest;
                            }
                            const dest = _path.default.join(distDir, _constants1.SERVER_DIRECTORY, relativeDest);
                            const isNotFound = prerenderManifest.notFoundRoutes.includes(page);
                            // for SSG files with i18n the non-prerendered variants are
                            // output with the locale prefixed so don't attempt moving
                            // without the prefix
                            if ((!i18n || additionalSsgFile) && !isNotFound) {
                                await _fs.promises.mkdir(_path.default.dirname(dest), {
                                    recursive: true
                                });
                                await _fs.promises.rename(orig, dest);
                            } else if (i18n && !isSsg) {
                                // this will be updated with the locale prefixed variant
                                // since all files are output with the locale prefix
                                delete pagesManifest[page];
                            }
                            if (i18n) {
                                if (additionalSsgFile) return;
                                const localeExt = page === '/' ? _path.default.extname(file) : '';
                                const relativeDestNoPages = relativeDest.slice('pages/'.length);
                                for (const locale of i18n.locales){
                                    const curPath = `/${locale}${page === '/' ? '' : page}`;
                                    if (isSsg && prerenderManifest.notFoundRoutes.includes(curPath)) {
                                        continue;
                                    }
                                    const updatedRelativeDest = _path.default.join('pages', locale + localeExt, // if it's the top-most index page we want it to be locale.EXT
                                    // instead of locale/index.html
                                    page === '/' ? '' : relativeDestNoPages).replace(/\\/g, '/');
                                    const updatedOrig = _path.default.join(outdir, locale + localeExt, page === '/' ? '' : file);
                                    const updatedDest = _path.default.join(distDir, _constants1.SERVER_DIRECTORY, updatedRelativeDest);
                                    if (!isSsg) {
                                        pagesManifest[curPath] = updatedRelativeDest;
                                    }
                                    await _fs.promises.mkdir(_path.default.dirname(updatedDest), {
                                        recursive: true
                                    });
                                    await _fs.promises.rename(updatedOrig, updatedDest);
                                }
                            }
                        });
                    };
                    async function moveExportedAppNotFoundTo404() {
                        return staticGenerationSpan.traceChild('move-exported-app-not-found-').traceAsyncFn(async ()=>{
                            const orig = _path.default.join(distDir, 'server', 'app', '_not-found.html');
                            const updatedRelativeDest = _path.default.join('pages', '404.html').replace(/\\/g, '/');
                            if ((0, _fs.existsSync)(orig)) {
                                // if 404.html folder doesn't exist, create it
                                await _fs.promises.mkdir(_path.default.dirname(_path.default.join(distDir, 'server', updatedRelativeDest)), {
                                    recursive: true
                                });
                                await _fs.promises.copyFile(orig, _path.default.join(distDir, 'server', updatedRelativeDest));
                                // since the app router not found is prioritized over pages router,
                                // we have to ensure the app router entries are available for all locales
                                if (i18n) {
                                    for (const locale of i18n.locales){
                                        const curPath = `/${locale}/404`;
                                        pagesManifest[curPath] = updatedRelativeDest;
                                    }
                                }
                                pagesManifest['/404'] = updatedRelativeDest;
                            }
                        });
                    }
                    async function moveExportedAppGlobalErrorTo500() {
                        return staticGenerationSpan.traceChild('move-exported-app-global-error-').traceAsyncFn(async ()=>{
                            // If static 500.html exists in pages router, don't move it
                            if ((0, _fs.existsSync)(_path.default.join(distDir, 'server', 'pages', '500.html'))) {
                                return;
                            }
                            // Only handle 500.html generation for static export
                            const orig = _path.default.join(distDir, 'server', 'app', '_global-error.html');
                            if ((0, _fs.existsSync)(orig)) {
                                const error500Html = _path.default.join(distDir, 'server', 'pages', '500.html');
                                // if 500.html folder doesn't exist, create it
                                await _fs.promises.mkdir(_path.default.dirname(error500Html), {
                                    recursive: true
                                });
                                await _fs.promises.copyFile(orig, error500Html);
                                pagesManifest['/500'] = _path.default.join('pages', '500.html').replace(/\\/g, '/');
                            }
                        });
                    }
                    // If there's /not-found inside app, we prefer it over the pages 404
                    if (hasStaticApp404) {
                        await moveExportedAppNotFoundTo404();
                    } else {
                        // Only move /404 to /404 when there is no custom 404 as in that case we don't know about the 404 page
                        if (!hasPages404 && !hasApp404 && useStaticPages404 && !appDirOnly) {
                            await moveExportedPage('/_error', '/404', '/404', false, 'html');
                        }
                    }
                    if (useDefaultStatic500 && !appDirOnly) {
                        await moveExportedPage('/_error', '/500', '/500', false, 'html');
                    }
                    // If there's app router and no pages router, use app router built-in 500.html
                    if (hasStaticAppGlobalError && mappedAppPages && Object.keys(mappedAppPages).length > 0) {
                        await moveExportedAppGlobalErrorTo500();
                    }
                    for (const page of combinedPages){
                        const isSsg = ssgPages.has(page);
                        const isStaticSsgFallback = ssgStaticFallbackPages.has(page);
                        const isDynamic = (0, _utils.isDynamicRoute)(page);
                        const file = (0, _normalizepagepath.normalizePagePath)(page);
                        const pageInfo = pageInfos.get(page);
                        const durationInfo = exportResult.byPage.get(page);
                        if (pageInfo && durationInfo) {
                            // Set Build Duration
                            if (pageInfo.ssgPageRoutes) {
                                pageInfo.ssgPageDurations = pageInfo.ssgPageRoutes.map((pagePath)=>{
                                    const duration = durationInfo.durationsByPath.get(pagePath);
                                    if (typeof duration === 'undefined') {
                                        throw Object.defineProperty(new Error("Invariant: page wasn't built"), "__NEXT_ERROR_CODE", {
                                            value: "E239",
                                            enumerable: false,
                                            configurable: true
                                        });
                                    }
                                    return duration;
                                });
                            }
                            pageInfo.pageDuration = durationInfo.durationsByPath.get(page);
                        }
                        // The dynamic version of SSG pages are only prerendered if the
                        // fallback is enabled. Below, we handle the specific prerenders
                        // of these.
                        const hasHtmlOutput = !(isSsg && isDynamic && !isStaticSsgFallback);
                        if (hasHtmlOutput) {
                            await moveExportedPage(page, page, file, isSsg, 'html');
                        }
                        if (isSsg) {
                            // For a non-dynamic SSG page, we must copy its data file
                            // from export, we already moved the HTML file above
                            if (!isDynamic) {
                                await moveExportedPage(page, page, file, isSsg, 'json');
                                if (i18n) {
                                    // TODO: do we want to show all locale variants in build output
                                    for (const locale of i18n.locales){
                                        const localePage = `/${locale}${page === '/' ? '' : page}`;
                                        const cacheControl = getCacheControl(localePage);
                                        prerenderManifest.routes[localePage] = {
                                            initialRevalidateSeconds: cacheControl.revalidate,
                                            initialExpireSeconds: cacheControl.expire,
                                            experimentalPPR: undefined,
                                            renderingMode: undefined,
                                            srcRoute: null,
                                            dataRoute: _path.default.posix.join('/_next/data', buildId, `${localePage}.json`),
                                            prefetchDataRoute: undefined,
                                            allowHeader: ALLOWED_HEADERS
                                        };
                                    }
                                } else {
                                    const cacheControl = getCacheControl(page);
                                    prerenderManifest.routes[page] = {
                                        initialRevalidateSeconds: cacheControl.revalidate,
                                        initialExpireSeconds: cacheControl.expire,
                                        experimentalPPR: undefined,
                                        renderingMode: undefined,
                                        srcRoute: null,
                                        dataRoute: _path.default.posix.join('/_next/data', buildId, `${file}.json`),
                                        // Pages does not have a prefetch data route.
                                        prefetchDataRoute: undefined,
                                        allowHeader: ALLOWED_HEADERS
                                    };
                                }
                                if (pageInfo) {
                                    pageInfo.initialCacheControl = getCacheControl(page);
                                }
                            } else {
                                // For a dynamic SSG page, we did not copy its data exports and only
                                // copy the fallback HTML file (if present).
                                // We must also copy specific versions of this page as defined by
                                // `getStaticPaths` (additionalSsgPaths).
                                for (const route of additionalPaths.get(page) ?? []){
                                    const pageFile = (0, _normalizepagepath.normalizePagePath)(route.pathname);
                                    await moveExportedPage(page, route.pathname, pageFile, isSsg, 'html', true);
                                    await moveExportedPage(page, route.pathname, pageFile, isSsg, 'json', true);
                                    const cacheControl = getCacheControl(route.pathname);
                                    prerenderManifest.routes[route.pathname] = {
                                        initialRevalidateSeconds: cacheControl.revalidate,
                                        initialExpireSeconds: cacheControl.expire,
                                        experimentalPPR: undefined,
                                        renderingMode: undefined,
                                        srcRoute: page,
                                        dataRoute: _path.default.posix.join('/_next/data', buildId, `${(0, _normalizepagepath.normalizePagePath)(route.pathname)}.json`),
                                        // Pages does not have a prefetch data route.
                                        prefetchDataRoute: undefined,
                                        allowHeader: ALLOWED_HEADERS
                                    };
                                    if (pageInfo) {
                                        pageInfo.initialCacheControl = cacheControl;
                                    }
                                }
                            }
                        }
                    }
                    // remove temporary export folder
                    await _fs.promises.rm(outdir, {
                        recursive: true,
                        force: true
                    });
                    await writeManifest(pagesManifestPath, pagesManifest);
                });
                // As we may have modified the dynamicRoutes, we need to sort the
                // dynamic routes by page.
                routesManifest.dynamicRoutes = (0, _sortableroutes.sortSortableRouteObjects)(dynamicRoutes, (route)=>({
                        // If the route is PPR enabled, and has an associated source page,
                        // use it. Otherwise fallback to the page which should be the same.
                        sourcePage: sourcePages.get(route.page) ?? route.page,
                        page: route.page
                    }));
                // Now write the routes manifest out.
                await nextBuildSpan.traceChild('write-routes-manifest').traceAsyncFn(()=>writeManifest(routesManifestPath, routesManifest));
            }
            const finalizingPageOptimizationStart = process.hrtime();
            const postBuildSpinner = (0, _spinner.default)('Finalizing page optimization');
            let buildTracesSpinner;
            let buildTracesStart;
            if (buildTracesPromise) {
                buildTracesStart = process.hrtime();
                buildTracesSpinner = (0, _spinner.default)('Collecting build traces');
            }
            // When output: export we want to end the worker later as it's still used for writeFullyStaticExport
            if (config.output !== 'export') {
                // ensure the worker is not left hanging
                staticWorker == null ? void 0 : staticWorker.end();
                staticWorker = undefined // Reset staticWorker to make sure it does not end in `finally`
                ;
            }
            const analysisEnd = process.hrtime(analysisBegin);
            telemetry.record((0, _events.eventBuildOptimize)(pagesPaths, {
                durationInSeconds: analysisEnd[0],
                staticPageCount: staticPages.size,
                staticPropsPageCount: ssgPages.size,
                serverPropsPageCount: serverPropsPages.size,
                ssrPageCount: pagesPaths.length - (staticPages.size + ssgPages.size + serverPropsPages.size),
                hasStatic404: useStaticPages404,
                hasReportWebVitals: (namedExports == null ? void 0 : namedExports.includes('reportWebVitals')) ?? false,
                rewritesCount: combinedRewrites.length,
                headersCount: headers.length,
                redirectsCount: redirects.length - 1,
                headersWithHasCount: headers.filter((r)=>!!r.has).length,
                rewritesWithHasCount: combinedRewrites.filter((r)=>!!r.has).length,
                redirectsWithHasCount: redirects.filter((r)=>!!r.has).length,
                middlewareCount: hasMiddlewareFile || hasProxyFile ? 1 : 0,
                totalAppPagesCount,
                staticAppPagesCount,
                serverAppPagesCount,
                edgeRuntimeAppCount,
                edgeRuntimePagesCount
            }));
            if (_buildcontext.NextBuildContext.telemetryState) {
                const events = (0, _events.eventBuildFeatureUsage)(_buildcontext.NextBuildContext.telemetryState.usages);
                telemetry.record(events);
                telemetry.record((0, _events.eventPackageUsedInGetServerSideProps)(_buildcontext.NextBuildContext.telemetryState.packagesUsedInServerSideProps));
                const useCacheTracker = _buildcontext.NextBuildContext.telemetryState.useCacheTracker;
                for (const [key, value] of Object.entries(useCacheTracker)){
                    telemetry.record((0, _events.eventBuildFeatureUsage)([
                        {
                            featureName: key,
                            invocationCount: value
                        }
                    ]));
                }
            }
            if (ssgPages.size > 0 || appDir) {
                var _config_i18n;
                tbdPrerenderRoutes.forEach((tbdRoute)=>{
                    const normalizedRoute = (0, _normalizepagepath.normalizePagePath)(tbdRoute);
                    const dataRoute = _path.default.posix.join('/_next/data', buildId, `${normalizedRoute}.json`);
                    prerenderManifest.dynamicRoutes[tbdRoute] = {
                        routeRegex: (0, _loadcustomroutes.normalizeRouteRegex)((0, _routeregex.getNamedRouteRegex)(tbdRoute, {
                            prefixRouteKeys: false
                        }).re.source),
                        experimentalPPR: undefined,
                        renderingMode: undefined,
                        dataRoute,
                        fallback: ssgBlockingFallbackPages.has(tbdRoute) ? null : ssgStaticFallbackPages.has(tbdRoute) ? `${normalizedRoute}.html` : false,
                        fallbackRevalidate: undefined,
                        fallbackExpire: undefined,
                        fallbackSourceRoute: undefined,
                        fallbackRootParams: undefined,
                        fallbackRouteParams: undefined,
                        dataRouteRegex: (0, _loadcustomroutes.normalizeRouteRegex)((0, _routeregex.getNamedRouteRegex)(dataRoute, {
                            prefixRouteKeys: true,
                            includeSuffix: true,
                            excludeOptionalTrailingSlash: true
                        }).re.source),
                        // Pages does not have a prefetch data route.
                        prefetchDataRoute: undefined,
                        prefetchDataRouteRegex: undefined,
                        allowHeader: ALLOWED_HEADERS
                    };
                });
                _buildcontext.NextBuildContext.previewModeId = previewProps.previewModeId;
                _buildcontext.NextBuildContext.fetchCacheKeyPrefix = config.experimental.fetchCacheKeyPrefix;
                _buildcontext.NextBuildContext.allowedRevalidateHeaderKeys = config.experimental.allowedRevalidateHeaderKeys;
                await writePrerenderManifest(distDir, prerenderManifest);
                await writeClientSsgManifest(prerenderManifest, {
                    distDir,
                    buildId,
                    locales: (_config_i18n = config.i18n) == null ? void 0 : _config_i18n.locales
                });
            } else {
                await writePrerenderManifest(distDir, {
                    version: 4,
                    routes: {},
                    dynamicRoutes: {},
                    preview: previewProps,
                    notFoundRoutes: []
                });
            }
            await writeImagesManifest(distDir, config);
            await writeManifest(_path.default.join(distDir, _constants1.EXPORT_MARKER), {
                version: 1,
                hasExportPathMap: typeof config.exportPathMap === 'function',
                exportTrailingSlash: config.trailingSlash === true,
                isNextImageImported: isNextImageImported === true
            });
            await _fs.promises.unlink(_path.default.join(distDir, _constants1.EXPORT_DETAIL)).catch((err)=>{
                if (err.code === 'ENOENT') {
                    return Promise.resolve();
                }
                return Promise.reject(err);
            });
            if (Boolean(config.experimental.nextScriptWorkers)) {
                await nextBuildSpan.traceChild('verify-partytown-setup').traceAsyncFn(async ()=>{
                    await (0, _verifypartytownsetup.verifyPartytownSetup)(dir, _path.default.join(distDir, _constants1.CLIENT_STATIC_FILES_PATH));
                });
            }
            await buildTracesPromise;
            if (buildTracesSpinner) {
                if (buildTracesStart) {
                    const buildTracesEnd = process.hrtime(buildTracesStart);
                    buildTracesSpinner.setText(`Collecting build traces in ${(0, _durationtostring.hrtimeDurationToString)(buildTracesEnd)}`);
                }
                buildTracesSpinner.stopAndPersist();
                buildTracesSpinner = undefined;
            }
            if (proxyFilePath && bundler !== _bundler.Bundler.Turbopack) {
                await _fs.promises.rename(_path.default.join(distDir, _constants1.SERVER_DIRECTORY, 'proxy.js'), _path.default.join(distDir, _constants1.SERVER_DIRECTORY, 'middleware.js'));
                await _fs.promises.rename(_path.default.join(distDir, _constants1.SERVER_DIRECTORY, 'proxy.js.nft.json'), _path.default.join(distDir, _constants1.SERVER_DIRECTORY, 'middleware.js.nft.json'));
                const middlewareNft = JSON.parse(await _fs.promises.readFile(_path.default.join(distDir, _constants1.SERVER_DIRECTORY, 'middleware.js.nft.json'), 'utf8'));
                // When Proxy self-reference itself e.g. __filename, it is traced to
                // the NFT file. However, since we rename 'proxy.js' to 'middleware.js',
                // the files in NFT will differ from the actual outputs, which will fail
                // for the providers like Vercel that uses NFT. Therefore also rename
                // the 'proxy.js' to 'middleware.js' in the NFT file.
                let hasProxyJsInNft = false;
                middlewareNft.files = middlewareNft.files.map((file)=>{
                    if (file === 'proxy.js') {
                        hasProxyJsInNft = true;
                        return 'middleware.js';
                    }
                    return file;
                });
                if (hasProxyJsInNft) {
                    await _fs.promises.writeFile(_path.default.join(distDir, _constants1.SERVER_DIRECTORY, 'middleware.js.nft.json'), JSON.stringify(middlewareNft));
                }
            }
            if (isCompileMode) {
                _log.info(`Build ran with "compile" mode, to finalize the build run either "generate" or "generate-env" mode as well`);
            }
            if (config.output === 'export') {
                // TODO: When writeFullyStaticExport doesn't fail when staticWorker is passed moved this after writeFullyStaticExport.
                // End the worker here when it's output: export.
                staticWorker.end();
                staticWorker = undefined // Reset staticWorker to make sure it does not end in `finally`
                ;
                await nextBuildSpan.traceChild('output-export-full-static-export').traceAsyncFn(async ()=>{
                    await writeFullyStaticExport(config, dir, enabledDirectories, configOutDir, nextBuildSpan, appDirOnly);
                });
            }
            // This should come after output: export handling but before
            // output: standalone, in the future output: standalone might
            // not be allowed if an adapter with onBuildComplete is configured
            const adapterPath = config.experimental.adapterPath;
            if (adapterPath) {
                await nextBuildSpan.traceChild('adapter-handle-build-complete').traceAsyncFn(async ()=>{
                    await (0, _buildcomplete.handleBuildComplete)({
                        dir,
                        distDir,
                        config,
                        buildId,
                        configOutDir: _path.default.join(dir, configOutDir),
                        staticPages,
                        serverPropsPages,
                        nextVersion: "16.1.6",
                        tracingRoot: outputFileTracingRoot,
                        hasNodeMiddleware,
                        hasInstrumentationHook,
                        adapterPath,
                        pageKeys: pageKeys.pages,
                        appPageKeys: denormalizedAppPages,
                        routesManifest,
                        prerenderManifest,
                        middlewareManifest,
                        functionsConfigManifest,
                        hasStatic404: useStaticPages404,
                        hasStatic500: useDefaultStatic500,
                        requiredServerFiles: requiredServerFilesManifest.files
                    });
                });
            }
            if (config.output === 'standalone') {
                await nextBuildSpan.traceChild('output-standalone').traceAsyncFn(async ()=>{
                    await writeStandaloneDirectory(nextBuildSpan, distDir, pageKeys, denormalizedAppPages, outputFileTracingRoot, requiredServerFilesManifest, middlewareManifest, hasNodeMiddleware, hasInstrumentationHook, staticPages, loadedEnvFiles, appDir);
                });
            }
            if (postBuildSpinner) {
                const finalizingPageOptimizationEnd = process.hrtime(finalizingPageOptimizationStart);
                postBuildSpinner.setText(`Finalizing page optimization in ${(0, _durationtostring.hrtimeDurationToString)(finalizingPageOptimizationEnd)}`);
                postBuildSpinner.stopAndPersist();
            }
            console.log();
            if (debugOutput) {
                nextBuildSpan.traceChild('print-custom-routes').traceFn(()=>(0, _utils1.printCustomRoutes)({
                        redirects,
                        rewrites,
                        headers
                    }));
            }
            await nextBuildSpan.traceChild('print-tree-view').traceAsyncFn(()=>(0, _utils1.printTreeView)(pageKeys, pageInfos, {
                    pagesDir,
                    useStaticPages404,
                    pageExtensions: config.pageExtensions,
                    buildManifest,
                    middlewareManifest,
                    functionsConfigManifest,
                    hasGSPAndRevalidateZero
                }));
            await nextBuildSpan.traceChild('telemetry-flush').traceAsyncFn(()=>telemetry.flush());
            await shutdownPromise;
            if (_buildcontext.NextBuildContext.analyze) {
                await (0, _promises.cp)(_path.default.join(__dirname, '../bundle-analyzer'), _path.default.join(dir, '.next/diagnostics/analyze'), {
                    recursive: true
                });
                await (0, _promises.mkdir)(_path.default.join(dir, '.next/diagnostics/analyze/data'), {
                    recursive: true
                });
                // Write an index of routes for the route picker
                await (0, _promises.writeFile)(_path.default.join(dir, '.next/diagnostics/analyze/data/routes.json'), JSON.stringify(routesManifest.dynamicRoutes.map((r)=>r.page).concat(routesManifest.staticRoutes.map((r)=>r.page)), null, 2));
            }
        });
    } catch (e) {
        const telemetry = _shared.traceGlobals.get('telemetry');
        if (telemetry) {
            telemetry.record((0, _events.eventBuildFailed)({
                bundler: getBundlerForTelemetry(bundler),
                errorCode: getErrorCodeForTelemetry(e),
                durationInSeconds: Math.floor((Date.now() - buildStartTime) / 1000)
            }));
        }
        throw e;
    } finally{
        // @ts-expect-error Existence of staticWorker is checked here intentionally.
        if (staticWorker) {
            staticWorker.end();
        }
        // Ensure we wait for lockfile patching if present
        await _swc.lockfilePatchPromise.cur;
        // Flush telemetry before finishing (waits for async operations like setTimeout in debug mode)
        const telemetry = _shared.traceGlobals.get('telemetry');
        if (telemetry) {
            await telemetry.flush();
        }
        // Ensure all traces are flushed before finishing the command
        await (0, _trace.flushAllTraces)();
        (0, _swc.teardownTraceSubscriber)();
        if (traceUploadUrl && loadedConfig) {
            (0, _uploadtrace.default)({
                traceUploadUrl,
                mode: 'build',
                projectDir: dir,
                distDir: loadedConfig.distDir,
                isTurboSession: bundler === _bundler.Bundler.Turbopack,
                sync: true
            });
        }
    }
}
function errorFromUnsupportedSegmentConfig() {
    _log.error(`Invalid segment configuration export detected. This can cause unexpected behavior from the configs not being applied. You should see the relevant failures in the logs above. Please fix them to continue.`);
    process.exit(1);
}
function getBundlerForTelemetry(bundler) {
    switch(bundler){
        case _bundler.Bundler.Turbopack:
            return 'turbopack';
        case _bundler.Bundler.Rspack:
            return 'rspack';
        case _bundler.Bundler.Webpack:
            return 'webpack';
        default:
            throw Object.defineProperty(new Error(`unknown bundler: ${bundler}`), "__NEXT_ERROR_CODE", {
                value: "E826",
                enumerable: false,
                configurable: true
            });
    }
}
function getErrorCodeForTelemetry(err) {
    const code = (0, _errortelemetryutils.extractNextErrorCode)(err);
    if (code != null) {
        return code;
    }
    if (err instanceof Error && 'code' in err && typeof err.code === 'string') {
        return err.code;
    }
    if (err instanceof Error) {
        return err.name;
    }
    return 'Unknown';
}

//# sourceMappingURL=index.js.map