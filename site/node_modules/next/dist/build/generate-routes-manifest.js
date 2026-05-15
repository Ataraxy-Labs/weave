"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "generateRoutesManifest", {
    enumerable: true,
    get: function() {
        return generateRoutesManifest;
    }
});
const _approuterheaders = require("../client/components/app-router-headers");
const _constants = require("../lib/constants");
const _utils = require("../shared/lib/router/utils");
const _buildcustomroute = require("../lib/build-custom-route");
const _utils1 = require("./utils");
const _sortableroutes = require("../shared/lib/router/utils/sortable-routes");
function generateRoutesManifest(options) {
    const { pageKeys, config, redirects, headers, rewrites, restrictedRedirectPaths, isAppPPREnabled, appType } = options;
    const sortedRoutes = (0, _sortableroutes.sortPages)([
        ...pageKeys.pages,
        ...pageKeys.app ?? []
    ]);
    const staticRoutes = [];
    const dynamicRoutes = [];
    /**
   * A map of all the pages to their sourcePage value. This is only used for
   * routes that have PPR enabled and clientSegmentEnabled is true.
   */ const sourcePages = new Map();
    for (const route of sortedRoutes){
        if ((0, _utils.isDynamicRoute)(route)) {
            dynamicRoutes.push((0, _utils1.pageToRoute)(route, // This property is only relevant when PPR is enabled.
            undefined));
        } else if (!(0, _utils1.isReservedPage)(route) || // don't consider /api reserved here
        route.match(/^\/(api(\/|$))/)) {
            staticRoutes.push((0, _utils1.pageToRoute)(route));
        }
    }
    const routesManifest = {
        version: 3,
        pages404: true,
        appType,
        caseSensitive: !!config.experimental.caseSensitiveRoutes,
        basePath: config.basePath,
        redirects: redirects.map((r)=>(0, _buildcustomroute.buildCustomRoute)('redirect', r, restrictedRedirectPaths)),
        headers: headers.map((r)=>(0, _buildcustomroute.buildCustomRoute)('header', r)),
        rewrites: {
            beforeFiles: rewrites.beforeFiles.map((r)=>(0, _buildcustomroute.buildCustomRoute)('rewrite', r)),
            afterFiles: rewrites.afterFiles.map((r)=>(0, _buildcustomroute.buildCustomRoute)('rewrite', r)),
            fallback: rewrites.fallback.map((r)=>(0, _buildcustomroute.buildCustomRoute)('rewrite', r))
        },
        dynamicRoutes,
        staticRoutes,
        dataRoutes: [],
        i18n: config.i18n || undefined,
        rsc: {
            header: _approuterheaders.RSC_HEADER,
            // This vary header is used as a default. It is technically re-assigned in `base-server`,
            // and may include an additional Vary option for `Next-URL`.
            varyHeader: `${_approuterheaders.RSC_HEADER}, ${_approuterheaders.NEXT_ROUTER_STATE_TREE_HEADER}, ${_approuterheaders.NEXT_ROUTER_PREFETCH_HEADER}, ${_approuterheaders.NEXT_ROUTER_SEGMENT_PREFETCH_HEADER}`,
            prefetchHeader: _approuterheaders.NEXT_ROUTER_PREFETCH_HEADER,
            didPostponeHeader: _approuterheaders.NEXT_DID_POSTPONE_HEADER,
            contentTypeHeader: _approuterheaders.RSC_CONTENT_TYPE_HEADER,
            suffix: _constants.RSC_SUFFIX,
            prefetchSegmentHeader: _approuterheaders.NEXT_ROUTER_SEGMENT_PREFETCH_HEADER,
            prefetchSegmentSuffix: _constants.RSC_SEGMENT_SUFFIX,
            prefetchSegmentDirSuffix: _constants.RSC_SEGMENTS_DIR_SUFFIX,
            clientParamParsing: config.cacheComponents ?? false,
            clientParamParsingOrigins: config.experimental.clientParamParsingOrigins,
            dynamicRSCPrerender: isAppPPREnabled && config.cacheComponents === true
        },
        rewriteHeaders: {
            pathHeader: _approuterheaders.NEXT_REWRITTEN_PATH_HEADER,
            queryHeader: _approuterheaders.NEXT_REWRITTEN_QUERY_HEADER
        },
        skipProxyUrlNormalize: config.skipProxyUrlNormalize,
        ppr: isAppPPREnabled ? {
            chain: {
                headers: {
                    [_constants.NEXT_RESUME_HEADER]: '1'
                }
            }
        } : undefined
    };
    return {
        routesManifest,
        dynamicRoutes,
        sourcePages
    };
}

//# sourceMappingURL=generate-routes-manifest.js.map