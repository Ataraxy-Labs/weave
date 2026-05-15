// TODO-APP: hydration warning
"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
require("./app-webpack");
const _nextdevtools = require("next/dist/compiled/next-devtools");
const _appbootstrap = require("./app-bootstrap");
const _stitchederror = require("../next-devtools/userspace/app/errors/stitched-error");
const _onrecoverableerror = require("./react-client-callbacks/on-recoverable-error");
// eslint-disable-next-line @next/internal/typechecked-require
const instrumentationHooks = require('../lib/require-instrumentation-client');
(0, _appbootstrap.appBootstrap)((assetPrefix)=>{
    const { hydrate } = require('./app-index');
    try {
        hydrate(instrumentationHooks, assetPrefix);
    } finally{
        const enableCacheIndicator = process.env.__NEXT_CACHE_COMPONENTS;
        (0, _nextdevtools.renderAppDevOverlay)(_stitchederror.getOwnerStack, _onrecoverableerror.isRecoverableError, enableCacheIndicator);
    }
});

if ((typeof exports.default === 'function' || (typeof exports.default === 'object' && exports.default !== null)) && typeof exports.default.__esModule === 'undefined') {
  Object.defineProperty(exports.default, '__esModule', { value: true });
  Object.assign(exports.default, exports);
  module.exports = exports.default;
}

//# sourceMappingURL=app-next-dev.js.map