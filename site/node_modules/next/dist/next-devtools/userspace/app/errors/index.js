"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
0 && (module.exports = {
    decorateDevError: null,
    handleClientError: null,
    originConsoleError: null
});
function _export(target, all) {
    for(var name in all)Object.defineProperty(target, name, {
        enumerable: true,
        get: all[name]
    });
}
_export(exports, {
    decorateDevError: function() {
        return _stitchederror.decorateDevError;
    },
    handleClientError: function() {
        return _useerrorhandler.handleClientError;
    },
    originConsoleError: function() {
        return _interceptconsoleerror.originConsoleError;
    }
});
const _interceptconsoleerror = require("./intercept-console-error");
const _useerrorhandler = require("./use-error-handler");
const _stitchederror = require("./stitched-error");

if ((typeof exports.default === 'function' || (typeof exports.default === 'object' && exports.default !== null)) && typeof exports.default.__esModule === 'undefined') {
  Object.defineProperty(exports.default, '__esModule', { value: true });
  Object.assign(exports.default, exports);
  module.exports = exports.default;
}

//# sourceMappingURL=index.js.map