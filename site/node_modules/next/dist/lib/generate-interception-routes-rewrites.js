"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
0 && (module.exports = {
    generateInterceptionRoutesRewrites: null,
    isInterceptionRouteRewrite: null
});
function _export(target, all) {
    for(var name in all)Object.defineProperty(target, name, {
        enumerable: true,
        get: all[name]
    });
}
_export(exports, {
    generateInterceptionRoutesRewrites: function() {
        return generateInterceptionRoutesRewrites;
    },
    isInterceptionRouteRewrite: function() {
        return isInterceptionRouteRewrite;
    }
});
const _approuterheaders = require("../client/components/app-router-headers");
const _interceptionroutes = require("../shared/lib/router/utils/interception-routes");
const _routeregex = require("../shared/lib/router/utils/route-regex");
function generateInterceptionRoutesRewrites(appPaths, basePath = '') {
    const rewrites = [];
    for (const appPath of appPaths){
        if ((0, _interceptionroutes.isInterceptionRouteAppPath)(appPath)) {
            const { interceptingRoute, interceptedRoute } = (0, _interceptionroutes.extractInterceptionRouteInformation)(appPath);
            const destination = (0, _routeregex.getNamedRouteRegex)(basePath + appPath, {
                prefixRouteKeys: true
            });
            const header = (0, _routeregex.getNamedRouteRegex)(interceptingRoute, {
                prefixRouteKeys: true,
                reference: destination.reference
            });
            const source = (0, _routeregex.getNamedRouteRegex)(basePath + interceptedRoute, {
                prefixRouteKeys: true,
                reference: header.reference
            });
            const headerRegex = header.namedRegex// Strip ^ and $ anchors since matchHas() will add them automatically
            .replace(/^\^/, '').replace(/\$$/, '')// Replace matching the `/` with matching any route segment.
            .replace(/^\/\(\?:\/\)\?$/, '/.*')// Replace the optional trailing with slash capture group with one that
            // will match any descendants.
            .replace(/\(\?:\/\)\?$/, '(?:/.*)?');
            rewrites.push({
                source: source.pathToRegexpPattern,
                destination: destination.pathToRegexpPattern,
                has: [
                    {
                        type: 'header',
                        key: _approuterheaders.NEXT_URL,
                        value: headerRegex
                    }
                ],
                regex: source.namedRegex
            });
        }
    }
    return rewrites;
}
function isInterceptionRouteRewrite(route) {
    var _route_has_, _route_has;
    // When we generate interception rewrites in the above implementation, we always do so with only a single `has` condition.
    return ((_route_has = route.has) == null ? void 0 : (_route_has_ = _route_has[0]) == null ? void 0 : _route_has_.key) === _approuterheaders.NEXT_URL;
}

//# sourceMappingURL=generate-interception-routes-rewrites.js.map