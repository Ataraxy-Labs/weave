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
const _interop_require_default = require("@swc/helpers/_/_interop_require_default");
const _hotreloadertypes = require("../../server/dev/hot-reloader-types");
const _hotreloaderpages = /*#__PURE__*/ _interop_require_default._(require("./hot-reloader/pages/hot-reloader-pages"));
const _websocket = require("./hot-reloader/pages/websocket");
let reloading = false;
const _default = ()=>{
    const devClient = (0, _hotreloaderpages.default)();
    devClient.subscribeToHmrEvent((message)=>{
        if (reloading) return;
        // Retrieve the router if it's available
        const router = window.next?.router;
        // Determine if we're on an error page or the router is not initialized
        const isOnErrorPage = !router || router.pathname === '/404' || router.pathname === '/_error';
        switch(message.type){
            case _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.RELOAD_PAGE:
                {
                    (0, _websocket.sendMessage)(JSON.stringify({
                        event: 'client-reload-page',
                        clientId: window.__nextDevClientId
                    }));
                    reloading = true;
                    return window.location.reload();
                }
            case _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.REMOVED_PAGE:
                {
                    const [page] = message.data;
                    // Check if the removed page is the current page
                    const isCurrentPage = page === router?.pathname;
                    // We enter here if the removed page is currently being viewed
                    // or if we happen to be on an error page.
                    if (isCurrentPage || isOnErrorPage) {
                        (0, _websocket.sendMessage)(JSON.stringify({
                            event: 'client-removed-page',
                            clientId: window.__nextDevClientId,
                            page
                        }));
                        return window.location.reload();
                    }
                    return;
                }
            case _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.ADDED_PAGE:
                {
                    const [page] = message.data;
                    // Check if the added page is the current page
                    const isCurrentPage = page === router?.pathname;
                    // Check if the page component is not yet loaded
                    const isPageNotLoaded = page !== null && typeof router?.components?.[page] === 'undefined';
                    // We enter this block if the newly added page is the one currently being viewed
                    // but hasn't been loaded yet, or if we're on an error page.
                    if (isCurrentPage && isPageNotLoaded || isOnErrorPage) {
                        (0, _websocket.sendMessage)(JSON.stringify({
                            event: 'client-added-page',
                            clientId: window.__nextDevClientId,
                            page
                        }));
                        return window.location.reload();
                    }
                    return;
                }
            case _hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.DEV_PAGES_MANIFEST_UPDATE:
                {
                    return;
                }
            default:
                {
                    message;
                }
        }
    });
    return devClient;
};

if ((typeof exports.default === 'function' || (typeof exports.default === 'object' && exports.default !== null)) && typeof exports.default.__esModule === 'undefined') {
  Object.defineProperty(exports.default, '__esModule', { value: true });
  Object.assign(exports.default, exports);
  module.exports = exports.default;
}

//# sourceMappingURL=hot-middleware-client.js.map