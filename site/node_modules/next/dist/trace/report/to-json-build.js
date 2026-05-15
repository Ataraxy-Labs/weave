"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "default", {
    enumerable: true,
    get: function() {
        return _default;
    }
});
const _shared = require("../shared");
const _fs = /*#__PURE__*/ _interop_require_default(require("fs"));
const _path = /*#__PURE__*/ _interop_require_default(require("path"));
const _constants = require("../../shared/lib/constants");
const _tojson = require("./to-json");
function _interop_require_default(obj) {
    return obj && obj.__esModule ? obj : {
        default: obj
    };
}
let writeStream;
let batch;
const writeStreamOptions = {
    flags: 'a',
    encoding: 'utf8'
};
class RotatingWriteStream {
    constructor(file, sizeLimit){
        this.file = file;
        this.size = 0;
        this.sizeLimit = sizeLimit;
        this.createWriteStream();
    }
    createWriteStream() {
        this.writeStream = _fs.default.createWriteStream(this.file, writeStreamOptions);
    }
    // Recreate the file
    async rotate() {
        await this.end();
        try {
            _fs.default.unlinkSync(this.file);
        } catch (err) {
            // It's fine if the file does not exist yet
            if (err.code !== 'ENOENT') {
                throw err;
            }
        }
        this.size = 0;
        this.createWriteStream();
        this.rotatePromise = undefined;
    }
    async write(data) {
        if (this.rotatePromise) await this.rotatePromise;
        this.size += data.length;
        if (this.size > this.sizeLimit) {
            await (this.rotatePromise = this.rotate());
        }
        if (!this.writeStream.write(data, 'utf8')) {
            if (this.drainPromise === undefined) {
                this.drainPromise = new Promise((resolve, _reject)=>{
                    this.writeStream.once('drain', ()=>{
                        this.drainPromise = undefined;
                        resolve();
                    });
                });
            }
            await this.drainPromise;
        }
    }
    end() {
        return new Promise((resolve)=>{
            this.writeStream.end(resolve);
        });
    }
}
const allowlistedEvents = new Set([
    'next-build',
    'run-turbopack',
    'run-webpack',
    'run-typescript',
    'run-eslint',
    'static-check',
    'collect-build-traces',
    'static-generation',
    'output-export-full-static-export',
    'adapter-handle-build-complete',
    'output-standalone',
    'telemetry-flush'
]);
function reportToJsonBuild(event) {
    if (!allowlistedEvents.has(event.name)) {
        return;
    }
    const distDir = _shared.traceGlobals.get('distDir');
    const phase = _shared.traceGlobals.get('phase');
    if (!distDir || !phase) {
        return;
    }
    // Only report in production builds
    if (phase !== _constants.PHASE_PRODUCTION_BUILD) {
        return;
    }
    if (!batch) {
        batch = (0, _tojson.batcher)(async (events)=>{
            if (!writeStream) {
                await _fs.default.promises.mkdir(distDir, {
                    recursive: true
                });
                const file = _path.default.join(distDir, 'trace-build');
                writeStream = new RotatingWriteStream(file, // Development is limited to 50MB, production is unlimited
                phase === _constants.PHASE_DEVELOPMENT_SERVER ? 52428800 : Infinity);
            }
            const eventsJson = JSON.stringify(events);
            try {
                await writeStream.write(eventsJson + '\n');
            } catch (err) {
                console.log(err);
            }
        });
    }
    batch.report({
        ...event,
        traceId: _shared.traceId
    });
}
const _default = {
    flushAll: (opts)=>batch ? batch.flushAll().then(()=>{
            const phase = _shared.traceGlobals.get('phase');
            // Only end writeStream when manually flushing in production
            if ((opts == null ? void 0 : opts.end) || phase !== _constants.PHASE_DEVELOPMENT_SERVER) {
                return writeStream.end();
            }
        }) : undefined,
    report: reportToJsonBuild
};

//# sourceMappingURL=to-json-build.js.map