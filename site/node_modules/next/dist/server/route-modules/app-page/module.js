"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
0 && (module.exports = {
    AppPageRouteModule: null,
    default: null,
    renderToHTMLOrFlight: null,
    vendored: null
});
function _export(target, all) {
    for(var name in all)Object.defineProperty(target, name, {
        enumerable: true,
        get: all[name]
    });
}
_export(exports, {
    AppPageRouteModule: function() {
        return AppPageRouteModule;
    },
    default: function() {
        return _default;
    },
    renderToHTMLOrFlight: function() {
        return _apprender.renderToHTMLOrFlight;
    },
    vendored: function() {
        return vendored;
    }
});
const _apprender = require("../../app-render/app-render");
const _routemodule = require("../route-module");
const _entrypoints = /*#__PURE__*/ _interop_require_wildcard(require("./vendored/contexts/entrypoints"));
const _prerendermanifestmatcher = require("./helpers/prerender-manifest-matcher");
const _approuterheaders = require("../../../client/components/app-router-headers");
const _interceptionroutes = require("../../../shared/lib/router/utils/interception-routes");
function _getRequireWildcardCache(nodeInterop) {
    if (typeof WeakMap !== "function") return null;
    var cacheBabelInterop = new WeakMap();
    var cacheNodeInterop = new WeakMap();
    return (_getRequireWildcardCache = function(nodeInterop) {
        return nodeInterop ? cacheNodeInterop : cacheBabelInterop;
    })(nodeInterop);
}
function _interop_require_wildcard(obj, nodeInterop) {
    if (!nodeInterop && obj && obj.__esModule) {
        return obj;
    }
    if (obj === null || typeof obj !== "object" && typeof obj !== "function") {
        return {
            default: obj
        };
    }
    var cache = _getRequireWildcardCache(nodeInterop);
    if (cache && cache.has(obj)) {
        return cache.get(obj);
    }
    var newObj = {
        __proto__: null
    };
    var hasPropertyDescriptor = Object.defineProperty && Object.getOwnPropertyDescriptor;
    for(var key in obj){
        if (key !== "default" && Object.prototype.hasOwnProperty.call(obj, key)) {
            var desc = hasPropertyDescriptor ? Object.getOwnPropertyDescriptor(obj, key) : null;
            if (desc && (desc.get || desc.set)) {
                Object.defineProperty(newObj, key, desc);
            } else {
                newObj[key] = obj[key];
            }
        }
    }
    newObj.default = obj;
    if (cache) {
        cache.set(obj, newObj);
    }
    return newObj;
}
let vendoredReactRSC;
let vendoredReactSSR;
// the vendored Reacts are loaded from their original source in the edge runtime
if (process.env.NEXT_RUNTIME !== 'edge') {
    vendoredReactRSC = require('./vendored/rsc/entrypoints');
    vendoredReactSSR = require('./vendored/ssr/entrypoints');
    // In Node environments we need to access the correct React instance from external modules such
    // as global patches. We register the loaded React instances here.
    const { registerServerReact, registerClientReact } = require('../../runtime-reacts.external');
    registerServerReact(vendoredReactRSC.React);
    registerClientReact(vendoredReactSSR.React);
}
class AppPageRouteModule extends _routemodule.RouteModule {
    match(pathname, prerenderManifest) {
        // Lazily create the matcher based on the provided prerender manifest.
        let matcher = this.matchers.get(prerenderManifest);
        if (!matcher) {
            matcher = new _prerendermanifestmatcher.PrerenderManifestMatcher(this.definition.pathname, prerenderManifest);
            this.matchers.set(prerenderManifest, matcher);
        }
        // Match the pathname to the dynamic route.
        return matcher.match(pathname);
    }
    render(req, res, context) {
        return (0, _apprender.renderToHTMLOrFlight)(req, res, context.page, context.query, context.fallbackRouteParams, context.renderOpts, context.serverComponentsHmrCache, context.sharedContext);
    }
    pathCouldBeIntercepted(resolvedPathname, interceptionRoutePatterns) {
        return (0, _interceptionroutes.isInterceptionRouteAppPath)(resolvedPathname) || interceptionRoutePatterns.some((regexp)=>{
            return regexp.test(resolvedPathname);
        });
    }
    getVaryHeader(resolvedPathname, interceptionRoutePatterns) {
        const baseVaryHeader = `${_approuterheaders.RSC_HEADER}, ${_approuterheaders.NEXT_ROUTER_STATE_TREE_HEADER}, ${_approuterheaders.NEXT_ROUTER_PREFETCH_HEADER}, ${_approuterheaders.NEXT_ROUTER_SEGMENT_PREFETCH_HEADER}`;
        if (this.pathCouldBeIntercepted(resolvedPathname, interceptionRoutePatterns)) {
            // Interception route responses can vary based on the `Next-URL` header.
            // We use the Vary header to signal this behavior to the client to properly cache the response.
            return `${baseVaryHeader}, ${_approuterheaders.NEXT_URL}`;
        } else {
            // We don't need to include `Next-URL` in the Vary header for non-interception routes since it won't affect the response.
            // We also set this header for pages to avoid caching issues when navigating between pages and app.
            return baseVaryHeader;
        }
    }
    constructor(...args){
        super(...args), this.matchers = new WeakMap();
    }
}
const vendored = {
    'react-rsc': vendoredReactRSC,
    'react-ssr': vendoredReactSSR,
    contexts: _entrypoints
};
const _default = AppPageRouteModule;

//# sourceMappingURL=module.js.map