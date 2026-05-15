"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
0 && (module.exports = {
    parseBuildPathsInput: null,
    resolveBuildPaths: null
});
function _export(target, all) {
    for(var name in all)Object.defineProperty(target, name, {
        enumerable: true,
        get: all[name]
    });
}
_export(exports, {
    parseBuildPathsInput: function() {
        return parseBuildPathsInput;
    },
    resolveBuildPaths: function() {
        return resolveBuildPaths;
    }
});
const _util = require("util");
const _glob = /*#__PURE__*/ _interop_require_default(require("next/dist/compiled/glob"));
const _log = /*#__PURE__*/ _interop_require_wildcard(require("../build/output/log"));
const _path = /*#__PURE__*/ _interop_require_default(require("path"));
const _fs = /*#__PURE__*/ _interop_require_default(require("fs"));
const _iserror = /*#__PURE__*/ _interop_require_default(require("./is-error"));
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
const glob = (0, _util.promisify)(_glob.default);
/**
 * Escapes bracket expressions that correspond to existing directories.
 * This allows Next.js dynamic routes like [slug] to work with glob patterns.
 *
 * e.g., "app/blog/[slug]/** /page.tsx" → "app/blog/\[slug\]/** /page.tsx"
 *       (if app/blog/[slug] directory exists)
 */ function escapeExistingBrackets(pattern, projectDir) {
    // Match bracket expressions: [name], [...name], [[...name]]
    const bracketRegex = /\[\[?\.\.\.[^\]]+\]?\]|\[[^\]]+\]/g;
    let lastIndex = 0;
    let result = '';
    let match;
    while((match = bracketRegex.exec(pattern)) !== null){
        const pathPrefix = pattern.slice(0, match.index + match[0].length);
        const exists = _fs.default.existsSync(_path.default.join(projectDir, pathPrefix));
        result += pattern.slice(lastIndex, match.index);
        result += exists ? match[0].replace(/\[/g, '\\[').replace(/\]/g, '\\]') : match[0];
        lastIndex = match.index + match[0].length;
    }
    return result + pattern.slice(lastIndex);
}
async function resolveBuildPaths(patterns, projectDir) {
    const appPaths = new Set();
    const pagePaths = new Set();
    for (const pattern of patterns){
        const trimmed = pattern.trim();
        if (!trimmed) continue;
        try {
            // Escape brackets that correspond to existing Next.js dynamic route directories
            const escapedPattern = escapeExistingBrackets(trimmed, projectDir);
            const matches = await glob(escapedPattern, {
                cwd: projectDir
            });
            if (matches.length === 0) {
                _log.warn(`Pattern "${trimmed}" did not match any files`);
            }
            for (const file of matches){
                if (!_fs.default.statSync(_path.default.join(projectDir, file)).isDirectory()) {
                    categorizeAndAddPath(file, appPaths, pagePaths);
                }
            }
        } catch (error) {
            throw Object.defineProperty(new Error(`Failed to resolve pattern "${trimmed}": ${(0, _iserror.default)(error) ? error.message : String(error)}`), "__NEXT_ERROR_CODE", {
                value: "E972",
                enumerable: false,
                configurable: true
            });
        }
    }
    return {
        appPaths: Array.from(appPaths).sort(),
        pagePaths: Array.from(pagePaths).sort()
    };
}
/**
 * Categorizes a file path to either app or pages router based on its prefix.
 *
 * Examples:
 * - "app/page.tsx" → appPaths.add("/page.tsx")
 * - "pages/index.tsx" → pagePaths.add("/index.tsx")
 */ function categorizeAndAddPath(filePath, appPaths, pagePaths) {
    const normalized = filePath.replace(/\\/g, '/');
    if (normalized.startsWith('app/')) {
        appPaths.add('/' + normalized.slice(4));
    } else if (normalized.startsWith('pages/')) {
        pagePaths.add('/' + normalized.slice(6));
    }
}
function parseBuildPathsInput(input) {
    // Comma-separated values
    return input.split(',').map((p)=>p.trim()).filter((p)=>p.length > 0);
}

//# sourceMappingURL=resolve-build-paths.js.map