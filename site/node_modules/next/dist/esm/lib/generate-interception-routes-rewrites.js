import { NEXT_URL } from '../client/components/app-router-headers';
import { extractInterceptionRouteInformation, isInterceptionRouteAppPath } from '../shared/lib/router/utils/interception-routes';
import { getNamedRouteRegex } from '../shared/lib/router/utils/route-regex';
export function generateInterceptionRoutesRewrites(appPaths, basePath = '') {
    const rewrites = [];
    for (const appPath of appPaths){
        if (isInterceptionRouteAppPath(appPath)) {
            const { interceptingRoute, interceptedRoute } = extractInterceptionRouteInformation(appPath);
            const destination = getNamedRouteRegex(basePath + appPath, {
                prefixRouteKeys: true
            });
            const header = getNamedRouteRegex(interceptingRoute, {
                prefixRouteKeys: true,
                reference: destination.reference
            });
            const source = getNamedRouteRegex(basePath + interceptedRoute, {
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
                        key: NEXT_URL,
                        value: headerRegex
                    }
                ],
                regex: source.namedRegex
            });
        }
    }
    return rewrites;
}
export function isInterceptionRouteRewrite(route) {
    var _route_has_, _route_has;
    // When we generate interception rewrites in the above implementation, we always do so with only a single `has` condition.
    return ((_route_has = route.has) == null ? void 0 : (_route_has_ = _route_has[0]) == null ? void 0 : _route_has_.key) === NEXT_URL;
}

//# sourceMappingURL=generate-interception-routes-rewrites.js.map