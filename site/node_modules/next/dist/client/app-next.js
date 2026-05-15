// This import must go first because it needs to patch webpack chunk loading
// before React patches chunk loading.
"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
require("./app-webpack");
const _appbootstrap = require("./app-bootstrap");
const instrumentationHooks = // eslint-disable-next-line @next/internal/typechecked-require -- not a module
require('../lib/require-instrumentation-client');
(0, _appbootstrap.appBootstrap)((assetPrefix)=>{
    const { hydrate } = require('./app-index');
    // Include app-router and layout-router in the main chunk
    // eslint-disable-next-line @next/internal/typechecked-require -- Why not relative imports?
    require('next/dist/client/components/app-router');
    // eslint-disable-next-line @next/internal/typechecked-require -- Why not relative imports?
    require('next/dist/client/components/layout-router');
    hydrate(instrumentationHooks, assetPrefix);
});

if ((typeof exports.default === 'function' || (typeof exports.default === 'object' && exports.default !== null)) && typeof exports.default.__esModule === 'undefined') {
  Object.defineProperty(exports.default, '__esModule', { value: true });
  Object.assign(exports.default, exports);
  module.exports = exports.default;
}

//# sourceMappingURL=app-next.js.map