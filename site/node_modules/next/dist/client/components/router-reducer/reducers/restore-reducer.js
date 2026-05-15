"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "restoreReducer", {
    enumerable: true,
    get: function() {
        return restoreReducer;
    }
});
const _createhreffromurl = require("../create-href-from-url");
const _computechangedpath = require("../compute-changed-path");
const _pprnavigations = require("../ppr-navigations");
const _navigatereducer = require("./navigate-reducer");
function restoreReducer(state, action) {
    // This action is used to restore the router state from the history state.
    // However, it's possible that the history state no longer contains the `FlightRouterState`.
    // We will copy over the internal state on pushState/replaceState events, but if a history entry
    // occurred before hydration, or if the user navigated to a hash using a regular anchor link,
    // the history state will not contain the `FlightRouterState`.
    // In this case, we'll continue to use the existing tree so the router doesn't get into an invalid state.
    let treeToRestore;
    let renderedSearch;
    const historyState = action.historyState;
    if (historyState) {
        treeToRestore = historyState.tree;
        renderedSearch = historyState.renderedSearch;
    } else {
        treeToRestore = state.tree;
        renderedSearch = state.renderedSearch;
    }
    const currentUrl = new URL(state.canonicalUrl, location.origin);
    const restoredUrl = action.url;
    const restoredCanonicalUrl = (0, _createhreffromurl.createHrefFromUrl)(restoredUrl);
    const restoredNextUrl = (0, _computechangedpath.extractPathFromFlightRouterState)(treeToRestore) ?? restoredUrl.pathname;
    const now = Date.now();
    const accumulation = {
        scrollableSegments: null,
        separateRefreshUrls: null
    };
    const task = (0, _pprnavigations.startPPRNavigation)(now, currentUrl, state.cache, state.tree, treeToRestore, _pprnavigations.FreshnessPolicy.HistoryTraversal, null, null, null, null, false, false, accumulation);
    if (task === null) {
        const mutable = {
            preserveCustomHistoryState: true
        };
        return (0, _navigatereducer.handleExternalUrl)(state, mutable, restoredCanonicalUrl, false);
    }
    (0, _pprnavigations.spawnDynamicRequests)(task, restoredUrl, restoredNextUrl, _pprnavigations.FreshnessPolicy.HistoryTraversal, accumulation);
    return {
        // Set canonical url
        canonicalUrl: restoredCanonicalUrl,
        renderedSearch,
        pushRef: {
            pendingPush: false,
            mpaNavigation: false,
            // Ensures that the custom history state that was set is preserved when applying this update.
            preserveCustomHistoryState: true
        },
        focusAndScrollRef: state.focusAndScrollRef,
        cache: task.node,
        // Restore provided tree
        tree: treeToRestore,
        nextUrl: restoredNextUrl,
        // TODO: We need to restore previousNextUrl, too, which represents the
        // Next-Url that was used to fetch the data. Anywhere we fetch using the
        // canonical URL, there should be a corresponding Next-Url.
        previousNextUrl: null,
        debugInfo: null
    };
}

if ((typeof exports.default === 'function' || (typeof exports.default === 'object' && exports.default !== null)) && typeof exports.default.__esModule === 'undefined') {
  Object.defineProperty(exports.default, '__esModule', { value: true });
  Object.assign(exports.default, exports);
  module.exports = exports.default;
}

//# sourceMappingURL=restore-reducer.js.map