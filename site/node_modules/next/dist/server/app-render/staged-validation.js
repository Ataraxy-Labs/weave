"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "anySegmentHasRuntimePrefetchEnabled", {
    enumerable: true,
    get: function() {
        return anySegmentHasRuntimePrefetchEnabled;
    }
});
const _appdirmodule = require("../lib/app-dir-module");
const _parseloadertree = require("../../shared/lib/router/utils/parse-loader-tree");
async function anySegmentHasRuntimePrefetchEnabled(tree) {
    const { mod: layoutOrPageMod } = await (0, _appdirmodule.getLayoutOrPageModule)(tree);
    // TODO(restart-on-cache-miss): Does this work correctly for client page/layout modules?
    const prefetchConfig = layoutOrPageMod ? layoutOrPageMod.unstable_prefetch : undefined;
    /** Whether this segment should use a runtime prefetch instead of a static prefetch. */ const hasRuntimePrefetch = (prefetchConfig == null ? void 0 : prefetchConfig.mode) === 'runtime';
    if (hasRuntimePrefetch) {
        return true;
    }
    const { parallelRoutes } = (0, _parseloadertree.parseLoaderTree)(tree);
    for(const parallelRouteKey in parallelRoutes){
        const parallelRoute = parallelRoutes[parallelRouteKey];
        const hasChildRuntimePrefetch = await anySegmentHasRuntimePrefetchEnabled(parallelRoute);
        if (hasChildRuntimePrefetch) {
            return true;
        }
    }
    return false;
}

//# sourceMappingURL=staged-validation.js.map