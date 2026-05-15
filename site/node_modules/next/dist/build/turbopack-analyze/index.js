"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
0 && (module.exports = {
    turbopackAnalyze: null,
    waitForShutdown: null
});
function _export(target, all) {
    for(var name in all)Object.defineProperty(target, name, {
        enumerable: true,
        get: all[name]
    });
}
_export(exports, {
    turbopackAnalyze: function() {
        return turbopackAnalyze;
    },
    waitForShutdown: function() {
        return waitForShutdown;
    }
});
const _path = /*#__PURE__*/ _interop_require_default(require("path"));
const _turbopackwarning = require("../../lib/turbopack-warning");
const _utils = require("../../shared/lib/turbopack/utils");
const _swc = require("../swc");
const _ciinfo = require("../../server/ci-info");
const _compilationevents = require("../../shared/lib/turbopack/compilation-events");
const _utils1 = require("../utils");
const _normalizepath = require("../../lib/normalize-path");
const _constants = require("../../shared/lib/constants");
function _interop_require_default(obj) {
    return obj && obj.__esModule ? obj : {
        default: obj
    };
}
async function turbopackAnalyze(analyzeContext) {
    var _config_experimental, _config_turbopack, _config_turbopack1, _config_experimental1;
    await (0, _turbopackwarning.validateTurboNextConfig)({
        dir: analyzeContext.dir,
        configPhase: _constants.PHASE_PRODUCTION_BUILD
    });
    const { config, dir, distDir, noMangling } = analyzeContext;
    const currentNodeJsVersion = process.versions.node;
    const startTime = process.hrtime();
    const bindings = await (0, _swc.loadBindings)(config == null ? void 0 : (_config_experimental = config.experimental) == null ? void 0 : _config_experimental.useWasmBinary);
    const dev = false;
    const supportedBrowsers = (0, _utils1.getSupportedBrowsers)(dir, dev);
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
            config,
            dev,
            distDir,
            projectPath: dir,
            fetchCacheKeyPrefix: config.experimental.fetchCacheKeyPrefix,
            hasRewrites: false,
            // Implemented separately in Turbopack, doesn't have to be passed here.
            middlewareMatchers: undefined,
            rewrites: {
                beforeFiles: [],
                afterFiles: [],
                fallback: []
            }
        }),
        buildId: 'analyze-build',
        encryptionKey: '',
        previewProps: {
            previewModeId: '',
            previewModeEncryptionKey: '',
            previewModeSigningKey: ''
        },
        browserslistQuery: supportedBrowsers.join(', '),
        noMangling,
        writeRoutesHashesManifest: false,
        currentNodeJsVersion
    }, {
        persistentCaching,
        memoryLimit: (_config_experimental1 = config.experimental) == null ? void 0 : _config_experimental1.turbopackMemoryLimit,
        dependencyTracking: persistentCaching,
        isCi: _ciinfo.isCI,
        isShortSession: true
    });
    try {
        (0, _compilationevents.backgroundLogCompilationEvents)(project);
        await project.writeAnalyzeData(analyzeContext.appDirOnly);
        const shutdownPromise = project.shutdown();
        const time = process.hrtime(startTime);
        return {
            duration: time[0] + time[1] / 1e9,
            shutdownPromise
        };
    } catch (err) {
        await project.shutdown();
        throw err;
    }
}
let shutdownPromise;
async function waitForShutdown() {
    if (shutdownPromise) {
        await shutdownPromise;
    }
}

//# sourceMappingURL=index.js.map