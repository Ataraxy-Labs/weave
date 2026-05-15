// imports polyfill from `@next/polyfill-module` after build.
"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
require("../build/polyfills/polyfill-module");
// Only setup devtools in development
if (process.env.NODE_ENV !== 'production') {
    require('../next-devtools/userspace/app/app-dev-overlay-setup');
}

if ((typeof exports.default === 'function' || (typeof exports.default === 'object' && exports.default !== null)) && typeof exports.default.__esModule === 'undefined') {
  Object.defineProperty(exports.default, '__esModule', { value: true });
  Object.assign(exports.default, exports);
  module.exports = exports.default;
}

//# sourceMappingURL=app-globals.js.map