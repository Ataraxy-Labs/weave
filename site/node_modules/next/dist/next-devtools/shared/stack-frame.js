"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
0 && (module.exports = {
    getFrameSource: null,
    getOriginalStackFrames: null
});
function _export(target, all) {
    for(var name in all)Object.defineProperty(target, name, {
        enumerable: true,
        get: all[name]
    });
}
_export(exports, {
    getFrameSource: function() {
        return getFrameSource;
    },
    getOriginalStackFrames: function() {
        return getOriginalStackFrames;
    }
});
const _webpackmodulepath = require("./webpack-module-path");
function getOriginalStackFrame(source, response) {
    async function _getOriginalStackFrame() {
        if (response.status === 'rejected') {
            throw Object.defineProperty(new Error(response.reason), "__NEXT_ERROR_CODE", {
                value: "E394",
                enumerable: false,
                configurable: true
            });
        }
        const body = response.value;
        return {
            error: false,
            reason: null,
            external: false,
            sourceStackFrame: source,
            originalStackFrame: body.originalStackFrame,
            originalCodeFrame: body.originalCodeFrame || null,
            ignored: body.originalStackFrame?.ignored || false
        };
    }
    // TODO: merge this section into ignoredList handling
    if (source.file === 'file://' || source.file?.match(/https?:\/\//)) {
        return Promise.resolve({
            error: false,
            reason: null,
            external: true,
            sourceStackFrame: source,
            originalStackFrame: null,
            originalCodeFrame: null,
            ignored: true
        });
    }
    return _getOriginalStackFrame().catch((err)=>({
            error: true,
            reason: err?.message ?? err?.toString() ?? 'Unknown Error',
            external: false,
            sourceStackFrame: source,
            originalStackFrame: null,
            originalCodeFrame: null,
            ignored: false
        }));
}
async function getOriginalStackFrames(frames, type, isAppDir) {
    const req = {
        frames,
        isServer: type === 'server',
        isEdgeServer: type === 'edge-server',
        isAppDirectory: isAppDir
    };
    let res = undefined;
    let reason = undefined;
    try {
        res = await fetch('/__nextjs_original-stack-frames', {
            method: 'POST',
            body: JSON.stringify(req)
        });
    } catch (e) {
        reason = e + '';
    }
    // When fails to fetch the original stack frames, we reject here to be
    // caught at `_getOriginalStackFrame()` and return the stack frames so
    // that the error overlay can render.
    if (res && res.ok && res.status !== 204) {
        const data = await res.json();
        return Promise.all(frames.map((frame, index)=>getOriginalStackFrame(frame, data[index])));
    } else {
        if (res) {
            reason = await res.text();
        }
    }
    return Promise.all(frames.map((frame)=>getOriginalStackFrame(frame, {
            status: 'rejected',
            reason: `Failed to fetch the original stack frames ${reason ? `: ${reason}` : ''}`
        })));
}
function getFrameSource(frame) {
    if (!frame.file) return '';
    const isWebpackFrame = (0, _webpackmodulepath.isWebpackInternalResource)(frame.file);
    let str = '';
    // Skip URL parsing for webpack internal file paths.
    if (isWebpackFrame) {
        str = (0, _webpackmodulepath.formatFrameSourceFile)(frame.file);
    } else {
        try {
            const u = new URL(frame.file);
            let parsedPath = '';
            // Strip the origin for same-origin scripts.
            if (globalThis.location?.origin !== u.origin) {
                // URLs can be valid without an `origin`, so long as they have a
                // `protocol`. However, `origin` is preferred.
                if (u.origin === 'null') {
                    parsedPath += u.protocol;
                } else {
                    parsedPath += u.origin;
                }
            }
            // Strip query string information as it's typically too verbose to be
            // meaningful.
            parsedPath += u.pathname;
            str = (0, _webpackmodulepath.formatFrameSourceFile)(parsedPath);
        } catch  {
            str = (0, _webpackmodulepath.formatFrameSourceFile)(frame.file);
        }
    }
    if (!(0, _webpackmodulepath.isWebpackInternalResource)(frame.file) && frame.line1 != null) {
        // We don't need line and column numbers for anonymous sources because
        // there's no entrypoint for the location anyway.
        if (str && frame.file !== '<anonymous>') {
            if (frame.column1 != null) {
                str += ` (${frame.line1}:${frame.column1})`;
            } else {
                str += ` (${frame.line1})`;
            }
        }
    }
    return str;
}

if ((typeof exports.default === 'function' || (typeof exports.default === 'object' && exports.default !== null)) && typeof exports.default.__esModule === 'undefined') {
  Object.defineProperty(exports.default, '__esModule', { value: true });
  Object.assign(exports.default, exports);
  module.exports = exports.default;
}

//# sourceMappingURL=stack-frame.js.map