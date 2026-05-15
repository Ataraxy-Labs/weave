"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "turbopackBuild", {
    enumerable: true,
    get: function() {
        return turbopackBuild;
    }
});
const _path = /*#__PURE__*/ _interop_require_default(require("path"));
const _worker = require("../../lib/worker");
const _buildcontext = require("../build-context");
function _interop_require_default(obj) {
    return obj && obj.__esModule ? obj : {
        default: obj
    };
}
async function turbopackBuildWithWorker() {
    try {
        const worker = new _worker.Worker(_path.default.join(__dirname, 'impl.js'), {
            exposedMethods: [
                'workerMain',
                'waitForShutdown'
            ],
            enableWorkerThreads: true,
            debuggerPortOffset: -1,
            isolatedMemory: false,
            numWorkers: 1,
            maxRetries: 0,
            forkOptions: {
                env: {
                    NEXT_PRIVATE_BUILD_WORKER: '1'
                }
            }
        });
        const { nextBuildSpan, // Config is not serializable and is loaded in the worker.
        config: _config, ...prunedBuildContext } = _buildcontext.NextBuildContext;
        const { buildTraceContext, duration } = await worker.workerMain({
            buildContext: prunedBuildContext
        });
        return {
            // destroy worker when Turbopack has shutdown so it's not sticking around using memory
            // We need to wait for shutdown to make sure filesystem cache is flushed
            shutdownPromise: worker.waitForShutdown().then(()=>{
                worker.end();
            }),
            buildTraceContext,
            duration
        };
    } catch (err) {
        // When the error is a serialized `Error` object we need to recreate the `Error` instance
        // in order to keep the consistent error reporting behavior.
        if (err.type === 'Error') {
            const error = Object.defineProperty(new Error(err.message), "__NEXT_ERROR_CODE", {
                value: "E394",
                enumerable: false,
                configurable: true
            });
            if (err.name) {
                error.name = err.name;
            }
            if (err.cause) {
                error.cause = err.cause;
            }
            error.message = err.message;
            error.stack = err.stack;
            throw error;
        }
        throw err;
    }
}
function turbopackBuild(withWorker) {
    const nextBuildSpan = _buildcontext.NextBuildContext.nextBuildSpan;
    return nextBuildSpan.traceChild('run-turbopack').traceAsyncFn(async ()=>{
        if (withWorker) {
            return await turbopackBuildWithWorker();
        } else {
            const build = require('./impl').turbopackBuild;
            return await build();
        }
    });
}

//# sourceMappingURL=index.js.map