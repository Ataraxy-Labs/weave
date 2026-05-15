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
const _nodepath = /*#__PURE__*/ _interop_require_wildcard(require("node:path"));
const _promises = /*#__PURE__*/ _interop_require_wildcard(require("node:fs/promises"));
const _apppaths = require("../../../shared/lib/router/utils/app-paths");
const _ensureleadingslash = require("../../../shared/lib/page-path/ensure-leading-slash");
const _getsegmentparam = require("../../../shared/lib/router/utils/get-segment-param");
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
const rootParamsLoader = async function() {
    const { appDir, pageExtensions } = this.getOptions();
    const allRootParams = await collectRootParamsFromFileSystem({
        appDir,
        pageExtensions
    });
    // invalidate the result whenever a file/directory is added/removed inside the app dir or its subdirectories,
    // because that might mean that a root layout has been moved.
    this.addContextDependency(appDir);
    // If there's no root params, there's nothing to generate.
    if (allRootParams.size === 0) {
        return 'export {}';
    }
    // Generate a getter for each root param we found.
    const sortedRootParamNames = Array.from(allRootParams).sort();
    const content = [
        `import { getRootParam } from 'next/dist/server/request/root-params';`,
        ...sortedRootParamNames.map((paramName)=>{
            return `export function ${paramName}() { return getRootParam('${paramName}'); }`;
        })
    ].join('\n');
    return content;
};
const _default = rootParamsLoader;
async function collectRootParamsFromFileSystem(opts) {
    return collectRootParams({
        appDir: opts.appDir,
        rootLayoutFilePaths: await findRootLayouts(opts)
    });
}
function collectRootParams({ rootLayoutFilePaths, appDir }) {
    const allRootParams = new Set();
    for (const rootLayoutFilePath of rootLayoutFilePaths){
        const params = getParamsFromLayoutFilePath({
            appDir,
            layoutFilePath: rootLayoutFilePath
        });
        for (const param of params){
            allRootParams.add(param);
        }
    }
    return allRootParams;
}
async function findRootLayouts({ appDir, pageExtensions }) {
    const layoutFilenameRegex = new RegExp(`^layout\\.(?:${pageExtensions.join('|')})$`);
    async function visit(directory) {
        let dir;
        try {
            dir = await _promises.readdir(directory, {
                withFileTypes: true
            });
        } catch (err) {
            // If the directory was removed before we managed to read it, just ignore it.
            if (err && typeof err === 'object' && 'code' in err && err.code === 'ENOENT') {
                return [];
            }
            throw err;
        }
        const subdirectories = [];
        for (const entry of dir){
            if (entry.isDirectory()) {
                // Directories that start with an underscore are excluded from routing, so we shouldn't look for layouts inside.
                if (entry.name[0] === '_') {
                    continue;
                }
                // Parallel routes cannot occur above a layout, so they can't contain a root layout.
                if (entry.name[0] === '@') {
                    continue;
                }
                const absolutePathname = _nodepath.join(directory, entry.name);
                subdirectories.push(absolutePathname);
            } else if (entry.isFile()) {
                if (layoutFilenameRegex.test(entry.name)) {
                    // We found a root layout, so we're not going to recurse into subdirectories,
                    // meaning that we can skip the rest of the entries.
                    // Note that we don't need to track any of the subdirectories as dependencies --
                    // changes in the subdirectories will only become relevant if this root layout is (re)moved,
                    // in which case the loader will re-run, traverse deeper (because it no longer stops at this root layout)
                    // and then track those directories as needed.
                    const rootLayoutPath = _nodepath.join(directory, entry.name);
                    return [
                        rootLayoutPath
                    ];
                }
            }
        }
        if (subdirectories.length === 0) {
            return [];
        }
        const subdirectoryRootLayouts = await Promise.all(subdirectories.map((subdirectory)=>visit(subdirectory)));
        return subdirectoryRootLayouts.flat(1);
    }
    return visit(appDir);
}
function getParamsFromLayoutFilePath({ appDir, layoutFilePath }) {
    const rootLayoutPath = (0, _apppaths.normalizeAppPath)((0, _ensureleadingslash.ensureLeadingSlash)(_nodepath.dirname(_nodepath.relative(appDir, layoutFilePath))));
    const segments = rootLayoutPath.split('/');
    const paramNames = [];
    for (const segment of segments){
        const param = (0, _getsegmentparam.getSegmentParam)(segment);
        if (param !== null) {
            paramNames.push(param.paramName);
        }
    }
    return paramNames;
}

//# sourceMappingURL=next-root-params-loader.js.map