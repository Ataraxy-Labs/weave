#!/usr/bin/env node
"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "nextBuild", {
    enumerable: true,
    get: function() {
        return nextBuild;
    }
});
require("../server/lib/cpu-profile");
const _fs = require("fs");
const _picocolors = require("../lib/picocolors");
const _build = /*#__PURE__*/ _interop_require_default(require("../build"));
const _log = require("../build/output/log");
const _utils = require("../server/lib/utils");
const _iserror = /*#__PURE__*/ _interop_require_default(require("../lib/is-error"));
const _getprojectdir = require("../lib/get-project-dir");
const _startup = require("../lib/memory/startup");
const _shutdown = require("../lib/memory/shutdown");
const _bundler = require("../lib/bundler");
const _resolvebuildpaths = require("../lib/resolve-build-paths");
function _interop_require_default(obj) {
    return obj && obj.__esModule ? obj : {
        default: obj
    };
}
const nextBuild = async (options, directory)=>{
    process.on('SIGTERM', ()=>process.exit(143));
    process.on('SIGINT', ()=>process.exit(130));
    const { experimentalAnalyze, debug, debugPrerender, experimentalDebugMemoryUsage, profile, mangling, experimentalAppOnly, experimentalBuildMode, experimentalUploadTrace, debugBuildPaths } = options;
    let traceUploadUrl;
    if (experimentalUploadTrace && !process.env.NEXT_TRACE_UPLOAD_DISABLED) {
        traceUploadUrl = experimentalUploadTrace;
    }
    const bundler = (0, _bundler.parseBundlerArgs)(options);
    if (experimentalAnalyze && bundler !== _bundler.Bundler.Turbopack) {
        (0, _utils.printAndExit)('--experimental-analyze is only compatible with the Turbopack bundler.');
    }
    if (!mangling) {
        (0, _log.warn)(`Mangling is disabled. ${(0, _picocolors.italic)('Note: This may affect performance and should only be used for debugging purposes.')}`);
    }
    if (profile) {
        (0, _log.warn)(`Profiling is enabled. ${(0, _picocolors.italic)('Note: This may affect performance.')}`);
    }
    if (debugPrerender) {
        (0, _log.warn)(`Prerendering is running in debug mode. ${(0, _picocolors.italic)('Note: This may affect performance and should not be used for production.')}`);
    }
    if (experimentalDebugMemoryUsage) {
        process.env.EXPERIMENTAL_DEBUG_MEMORY_USAGE = '1';
        (0, _startup.enableMemoryDebuggingMode)();
    }
    const dir = (0, _getprojectdir.getProjectDir)(directory);
    if (!(0, _fs.existsSync)(dir)) {
        (0, _utils.printAndExit)(`> No such directory exists as the project root: ${dir}`);
    }
    // Resolve selective build paths
    let resolvedAppPaths;
    let resolvedPagePaths;
    if (debugBuildPaths) {
        try {
            const patterns = (0, _resolvebuildpaths.parseBuildPathsInput)(debugBuildPaths);
            if (patterns.length > 0) {
                const resolved = await (0, _resolvebuildpaths.resolveBuildPaths)(patterns, dir);
                // Pass empty arrays to indicate "build nothing" vs undefined for "build everything"
                resolvedAppPaths = resolved.appPaths;
                resolvedPagePaths = resolved.pagePaths;
            }
        } catch (err) {
            (0, _utils.printAndExit)(`Failed to resolve build paths: ${(0, _iserror.default)(err) ? err.message : String(err)}`);
        }
    }
    return (0, _build.default)(dir, experimentalAnalyze, profile, debug || Boolean(process.env.NEXT_DEBUG_BUILD), debugPrerender, !mangling, experimentalAppOnly, bundler, experimentalBuildMode, traceUploadUrl, resolvedAppPaths, resolvedPagePaths).catch((err)=>{
        if (experimentalDebugMemoryUsage) {
            (0, _shutdown.disableMemoryDebuggingMode)();
        }
        console.error('');
        if ((0, _iserror.default)(err) && (err.code === 'INVALID_RESOLVE_ALIAS' || err.code === 'WEBPACK_ERRORS' || err.code === 'BUILD_OPTIMIZATION_FAILED' || err.code === 'NEXT_EXPORT_ERROR' || err.code === 'NEXT_STATIC_GEN_BAILOUT' || err.code === 'EDGE_RUNTIME_UNSUPPORTED_API')) {
            (0, _utils.printAndExit)(`> ${err.message}`);
        } else {
            console.error('> Build error occurred');
            (0, _utils.printAndExit)(err);
        }
    }).finally(()=>{
        if (experimentalDebugMemoryUsage) {
            (0, _shutdown.disableMemoryDebuggingMode)();
        }
    });
};

//# sourceMappingURL=next-build.js.map