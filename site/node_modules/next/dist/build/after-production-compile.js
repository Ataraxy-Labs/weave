"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "runAfterProductionCompile", {
    enumerable: true,
    get: function() {
        return runAfterProductionCompile;
    }
});
const _log = /*#__PURE__*/ _interop_require_wildcard(require("./output/log"));
const _spinner = /*#__PURE__*/ _interop_require_default(require("./spinner"));
const _iserror = /*#__PURE__*/ _interop_require_default(require("../lib/is-error"));
const _build = require("../telemetry/events/build");
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
async function runAfterProductionCompile({ config, buildSpan, telemetry, metadata }) {
    const run = config.compiler.runAfterProductionCompile;
    if (!run) {
        return;
    }
    telemetry.record([
        {
            eventName: _build.EVENT_BUILD_FEATURE_USAGE,
            payload: {
                featureName: 'runAfterProductionCompile',
                invocationCount: 1
            }
        }
    ]);
    const afterBuildSpinner = (0, _spinner.default)('Running next.config.js provided runAfterProductionCompile');
    try {
        const startTime = performance.now();
        await buildSpan.traceChild('after-production-compile').traceAsyncFn(async ()=>{
            await run(metadata);
        });
        const duration = performance.now() - startTime;
        const formattedDuration = `${Math.round(duration)}ms`;
        _log.event(`Completed runAfterProductionCompile in ${formattedDuration}`);
    } catch (err) {
        // Handle specific known errors differently if needed
        if ((0, _iserror.default)(err)) {
            _log.error(`Failed to run runAfterProductionCompile: ${err.message}`);
        }
        throw err;
    } finally{
        afterBuildSpinner == null ? void 0 : afterBuildSpinner.stop();
    }
}

//# sourceMappingURL=after-production-compile.js.map