"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "pageBootstrap", {
    enumerable: true,
    get: function() {
        return pageBootstrap;
    }
});
const _interop_require_default = require("@swc/helpers/_/_interop_require_default");
require("../lib/require-instrumentation-client");
const _ = require("./");
const _ondemandentriesclient = /*#__PURE__*/ _interop_require_default._(require("./dev/on-demand-entries-client"));
const _fouc = require("./dev/fouc");
const _websocket = require("./dev/hot-reloader/pages/websocket");
const _querystring = require("../shared/lib/router/utils/querystring");
const _hotreloadertypes = require("../server/dev/hot-reloader-types");
const _runtimeerrorhandler = require("./dev/runtime-error-handler");
const _shared = require("./dev/hot-reloader/shared");
const _hotreloaderpages = require("./dev/hot-reloader/pages/hot-reloader-pages");
const _nextdevtools = require("next/dist/compiled/next-devtools");
function pageBootstrap(assetPrefix) {
    (0, _websocket.connectHMR)({
        assetPrefix,
        path: '/_next/webpack-hmr'
    });
    return (0, _.hydrate)({
        beforeRender: _fouc.displayContent
    }).then(()=>{
        (0, _ondemandentriesclient.default)();
        let reloading = false;
        (0, _websocket.addMessageListener)((message)=>{
            if (reloading) return;
            switch(message.type){
                case _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.SERVER_ERROR:
                    {
                        const errorObject = JSON.parse(message.errorJSON);
                        const error = Object.defineProperty(new Error(errorObject.message), "__NEXT_ERROR_CODE", {
                            value: "E394",
                            enumerable: false,
                            configurable: true
                        });
                        error.stack = errorObject.stack;
                        throw error;
                    }
                case _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.RELOAD_PAGE:
                    {
                        reloading = true;
                        window.location.reload();
                        break;
                    }
                case _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.DEV_PAGES_MANIFEST_UPDATE:
                    {
                        fetch(`${assetPrefix}/_next/static/development/_devPagesManifest.json`).then((res)=>res.json()).then((manifest)=>{
                            window.__DEV_PAGES_MANIFEST = manifest;
                        }).catch((err)=>{
                            console.log(`Failed to fetch devPagesManifest`, err);
                        });
                        break;
                    }
                case _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.MIDDLEWARE_CHANGES:
                    {
                        return window.location.reload();
                    }
                case _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.CLIENT_CHANGES:
                    {
                        // This is used in `../server/dev/turbopack-utils.ts`.
                        const isOnErrorPage = window.next.router.pathname === '/_error';
                        // On the error page we want to reload the page when a page was changed
                        if (isOnErrorPage) {
                            if (_runtimeerrorhandler.RuntimeErrorHandler.hadRuntimeError) {
                                console.warn(_shared.REACT_REFRESH_FULL_RELOAD_FROM_ERROR);
                            }
                            reloading = true;
                            (0, _hotreloaderpages.performFullReload)(null);
                        }
                        break;
                    }
                case _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.SERVER_ONLY_CHANGES:
                    {
                        if (_runtimeerrorhandler.RuntimeErrorHandler.hadRuntimeError) {
                            console.warn(_shared.REACT_REFRESH_FULL_RELOAD_FROM_ERROR);
                            (0, _hotreloaderpages.performFullReload)(null);
                        }
                        const { pages } = message;
                        // Make sure to reload when the dev-overlay is showing for an
                        // API route
                        // TODO: Fix `__NEXT_PAGE` type
                        if (pages.includes(_.router.query.__NEXT_PAGE)) {
                            return window.location.reload();
                        }
                        if (!_.router.clc && pages.includes(_.router.pathname)) {
                            console.log('Refreshing page data due to server-side change');
                            _nextdevtools.dispatcher.buildingIndicatorShow();
                            const clearIndicator = _nextdevtools.dispatcher.buildingIndicatorHide;
                            _.router.replace(_.router.pathname + '?' + String((0, _querystring.assign)((0, _querystring.urlQueryToSearchParams)(_.router.query), new URLSearchParams(location.search))), _.router.asPath, {
                                scroll: false
                            }).catch(()=>{
                                // trigger hard reload when failing to refresh data
                                // to show error overlay properly
                                location.reload();
                            }).finally(clearIndicator);
                        }
                        break;
                    }
                case _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.ADDED_PAGE:
                case _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.REMOVED_PAGE:
                case _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.SERVER_COMPONENT_CHANGES:
                case _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.SYNC:
                case _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.BUILT:
                case _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.BUILDING:
                case _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.TURBOPACK_MESSAGE:
                case _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.TURBOPACK_CONNECTED:
                case _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.ISR_MANIFEST:
                case _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.DEVTOOLS_CONFIG:
                case _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.REACT_DEBUG_CHUNK:
                case _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.REQUEST_CURRENT_ERROR_STATE:
                case _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.REQUEST_PAGE_METADATA:
                case _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.CACHE_INDICATOR:
                case _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.ERRORS_TO_SHOW_IN_BROWSER:
                    break;
                default:
                    message;
            }
        });
    });
}

if ((typeof exports.default === 'function' || (typeof exports.default === 'object' && exports.default !== null)) && typeof exports.default.__esModule === 'undefined') {
  Object.defineProperty(exports.default, '__esModule', { value: true });
  Object.assign(exports.default, exports);
  module.exports = exports.default;
}

//# sourceMappingURL=page-bootstrap.js.map