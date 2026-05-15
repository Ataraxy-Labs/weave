"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
const _interceptconsoleerror = require("./errors/intercept-console-error");
const _useerrorhandler = require("./errors/use-error-handler");
const _forwardlogs = require("./forward-logs");
(0, _useerrorhandler.handleGlobalErrors)();
(0, _interceptconsoleerror.patchConsoleError)();
(0, _forwardlogs.initializeDebugLogForwarding)('app');

if ((typeof exports.default === 'function' || (typeof exports.default === 'object' && exports.default !== null)) && typeof exports.default.__esModule === 'undefined') {
  Object.defineProperty(exports.default, '__esModule', { value: true });
  Object.assign(exports.default, exports);
  module.exports = exports.default;
}

//# sourceMappingURL=app-dev-overlay-setup.js.map