"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
0 && (module.exports = {
    refreshDynamicData: null,
    refreshReducer: null
});
function _export(target, all) {
    for(var name in all)Object.defineProperty(target, name, {
        enumerable: true,
        get: all[name]
    });
}
_export(exports, {
    refreshDynamicData: function() {
        return refreshDynamicData;
    },
    refreshReducer: function() {
        return refreshReducer;
    }
});
const _navigatereducer = require("./navigate-reducer");
const _navigation = require("../../segment-cache/navigation");
const _cache = require("../../segment-cache/cache");
const _hasinterceptionrouteincurrenttree = require("./has-interception-route-in-current-tree");
const _pprnavigations = require("../ppr-navigations");
function refreshReducer(state) {
    // TODO: Currently, all refreshes purge the prefetch cache. In the future,
    // only client-side refreshes will have this behavior; the server-side
    // `refresh` should send new data without purging the prefetch cache.
    const currentNextUrl = state.nextUrl;
    const currentRouterState = state.tree;
    (0, _cache.revalidateEntireCache)(currentNextUrl, currentRouterState);
    return refreshDynamicData(state, _pprnavigations.FreshnessPolicy.RefreshAll);
}
function refreshDynamicData(state, freshnessPolicy) {
    const currentNextUrl = state.nextUrl;
    // We always send the last next-url, not the current when performing a dynamic
    // request. This is because we update the next-url after a navigation, but we
    // want the same interception route to be matched that used the last next-url.
    const nextUrlForRefresh = (0, _hasinterceptionrouteincurrenttree.hasInterceptionRouteInCurrentTree)(state.tree) ? state.previousNextUrl || currentNextUrl : null;
    // A refresh is modeled as a navigation to the current URL, but where any
    // existing dynamic data (including in shared layouts) is re-fetched.
    const currentCanonicalUrl = state.canonicalUrl;
    const currentUrl = new URL(currentCanonicalUrl, location.origin);
    const currentFlightRouterState = state.tree;
    const shouldScroll = true;
    const navigationSeed = {
        tree: state.tree,
        renderedSearch: state.renderedSearch,
        data: null,
        head: null
    };
    const now = Date.now();
    const result = (0, _navigation.navigateToSeededRoute)(now, currentUrl, currentCanonicalUrl, navigationSeed, currentUrl, state.cache, currentFlightRouterState, freshnessPolicy, nextUrlForRefresh, shouldScroll);
    const mutable = {};
    mutable.preserveCustomHistoryState = false;
    return (0, _navigatereducer.handleNavigationResult)(currentUrl, state, mutable, false, result);
}

if ((typeof exports.default === 'function' || (typeof exports.default === 'object' && exports.default !== null)) && typeof exports.default.__esModule === 'undefined') {
  Object.defineProperty(exports.default, '__esModule', { value: true });
  Object.assign(exports.default, exports);
  module.exports = exports.default;
}

//# sourceMappingURL=refresh-reducer.js.map