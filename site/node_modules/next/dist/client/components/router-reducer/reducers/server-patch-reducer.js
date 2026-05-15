"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "serverPatchReducer", {
    enumerable: true,
    get: function() {
        return serverPatchReducer;
    }
});
const _createhreffromurl = require("../create-href-from-url");
const _navigatereducer = require("./navigate-reducer");
const _navigation = require("../../segment-cache/navigation");
const _refreshreducer = require("./refresh-reducer");
const _pprnavigations = require("../ppr-navigations");
function serverPatchReducer(state, action) {
    const mutable = {};
    mutable.preserveCustomHistoryState = false;
    // A "retry" is a navigation that happens due to a route mismatch. It's
    // similar to a refresh, because we will omit any existing dynamic data on
    // the page. But we seed the retry navigation with the exact tree that the
    // server just responded with.
    const retryMpa = action.mpa;
    const retryUrl = new URL(action.url, location.origin);
    const retrySeed = action.seed;
    if (retryMpa || retrySeed === null) {
        // If the server did not send back data during the mismatch, fall back to
        // an MPA navigation.
        return (0, _navigatereducer.handleExternalUrl)(state, mutable, retryUrl.href, false);
    }
    const currentUrl = new URL(state.canonicalUrl, location.origin);
    if (action.previousTree !== state.tree) {
        // There was another, more recent navigation since the once that
        // mismatched. We can abort the retry, but we still need to refresh the
        // page to evict any stale dynamic data.
        return (0, _refreshreducer.refreshReducer)(state);
    }
    // There have been no new navigations since the mismatched one. Refresh,
    // using the tree we just received from the server.
    const retryCanonicalUrl = (0, _createhreffromurl.createHrefFromUrl)(retryUrl);
    const retryNextUrl = action.nextUrl;
    // A retry should not create a new history entry.
    const pendingPush = false;
    const shouldScroll = true;
    const now = Date.now();
    const result = (0, _navigation.navigateToSeededRoute)(now, retryUrl, retryCanonicalUrl, retrySeed, currentUrl, state.cache, state.tree, _pprnavigations.FreshnessPolicy.RefreshAll, retryNextUrl, shouldScroll);
    return (0, _navigatereducer.handleNavigationResult)(retryUrl, state, mutable, pendingPush, result);
}

if ((typeof exports.default === 'function' || (typeof exports.default === 'object' && exports.default !== null)) && typeof exports.default.__esModule === 'undefined') {
  Object.defineProperty(exports.default, '__esModule', { value: true });
  Object.assign(exports.default, exports);
  module.exports = exports.default;
}

//# sourceMappingURL=server-patch-reducer.js.map