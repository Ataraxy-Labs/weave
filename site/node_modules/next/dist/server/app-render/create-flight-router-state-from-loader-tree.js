"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
0 && (module.exports = {
    createFlightRouterStateFromLoaderTree: null,
    createRouteTreePrefetch: null
});
function _export(target, all) {
    for(var name in all)Object.defineProperty(target, name, {
        enumerable: true,
        get: all[name]
    });
}
_export(exports, {
    createFlightRouterStateFromLoaderTree: function() {
        return createFlightRouterStateFromLoaderTree;
    },
    createRouteTreePrefetch: function() {
        return createRouteTreePrefetch;
    }
});
const _approutertypes = require("../../shared/lib/app-router-types");
const _segment = require("../../shared/lib/segment");
function createFlightRouterStateFromLoaderTreeImpl([segment, parallelRoutes, { layout, loading }], getDynamicParamFromSegment, searchParams, includeHasLoadingBoundary, didFindRootLayout) {
    const dynamicParam = getDynamicParamFromSegment(segment);
    const treeSegment = dynamicParam ? dynamicParam.treeSegment : segment;
    const segmentTree = [
        (0, _segment.addSearchParamsIfPageSegment)(treeSegment, searchParams),
        {}
    ];
    // Mark the first segment that has a layout as the "root" layout
    if (!didFindRootLayout && typeof layout !== 'undefined') {
        didFindRootLayout = true;
        segmentTree[4] = true;
    }
    let childHasLoadingBoundary = false;
    const children = {};
    Object.keys(parallelRoutes).forEach((parallelRouteKey)=>{
        const child = createFlightRouterStateFromLoaderTreeImpl(parallelRoutes[parallelRouteKey], getDynamicParamFromSegment, searchParams, includeHasLoadingBoundary, didFindRootLayout);
        if (includeHasLoadingBoundary && child[5] !== _approutertypes.HasLoadingBoundary.SubtreeHasNoLoadingBoundary) {
            childHasLoadingBoundary = true;
        }
        children[parallelRouteKey] = child;
    });
    segmentTree[1] = children;
    if (includeHasLoadingBoundary) {
        // During a route tree prefetch, the FlightRouterState includes whether a
        // tree has a loading boundary. The client uses this to determine if it can
        // skip the data prefetch request — if `hasLoadingBoundary` is `false`, the
        // data prefetch response will be empty, so there's no reason to request it.
        // NOTE: It would be better to accumulate this while building the loader
        // tree so we don't have to keep re-deriving it, but since this won't be
        // once PPR is enabled everywhere, it's not that important.
        segmentTree[5] = loading ? _approutertypes.HasLoadingBoundary.SegmentHasLoadingBoundary : childHasLoadingBoundary ? _approutertypes.HasLoadingBoundary.SubtreeHasLoadingBoundary : _approutertypes.HasLoadingBoundary.SubtreeHasNoLoadingBoundary;
    }
    return segmentTree;
}
function createFlightRouterStateFromLoaderTree(loaderTree, getDynamicParamFromSegment, searchParams) {
    const includeHasLoadingBoundary = false;
    const didFindRootLayout = false;
    return createFlightRouterStateFromLoaderTreeImpl(loaderTree, getDynamicParamFromSegment, searchParams, includeHasLoadingBoundary, didFindRootLayout);
}
function createRouteTreePrefetch(loaderTree, getDynamicParamFromSegment) {
    // Search params should not be added to page segment's cache key during a
    // route tree prefetch request, because they do not affect the structure of
    // the route. The client cache has its own logic to handle search params.
    const searchParams = {};
    // During a route tree prefetch, we include `hasLoadingBoundary` in
    // the response.
    const includeHasLoadingBoundary = true;
    const didFindRootLayout = false;
    return createFlightRouterStateFromLoaderTreeImpl(loaderTree, getDynamicParamFromSegment, searchParams, includeHasLoadingBoundary, didFindRootLayout);
}

//# sourceMappingURL=create-flight-router-state-from-loader-tree.js.map