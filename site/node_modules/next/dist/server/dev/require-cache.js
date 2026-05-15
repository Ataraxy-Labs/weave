"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "deleteCache", {
    enumerable: true,
    get: function() {
        return deleteCache;
    }
});
const _iserror = /*#__PURE__*/ _interop_require_default(require("../../lib/is-error"));
const _realpath = require("../../lib/realpath");
const _loadmanifestexternal = require("../load-manifest.external");
function _interop_require_default(obj) {
    return obj && obj.__esModule ? obj : {
        default: obj
    };
}
function deleteFromRequireCache(filePath) {
    try {
        filePath = (0, _realpath.realpathSync)(filePath);
    } catch (e) {
        if ((0, _iserror.default)(e) && e.code !== 'ENOENT') throw e;
    }
    const mod = require.cache[filePath];
    if (mod) {
        // remove the child reference from all parent modules
        for (const parent of Object.values(require.cache)){
            if (parent == null ? void 0 : parent.children) {
                const idx = parent.children.indexOf(mod);
                if (idx >= 0) parent.children.splice(idx, 1);
            }
        }
        // remove parent references from external modules
        for (const child of mod.children){
            child.parent = null;
        }
        delete require.cache[filePath];
        return true;
    }
    return false;
}
function deleteCache(filePath) {
    // try to clear it from the fs cache
    (0, _loadmanifestexternal.clearManifestCache)(filePath);
    deleteFromRequireCache(filePath);
}

//# sourceMappingURL=require-cache.js.map