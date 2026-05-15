import { getRouteMatcher } from '../../../../shared/lib/router/utils/route-matcher';
import { getRouteRegex } from '../../../../shared/lib/router/utils/route-regex';
/**
 * A matcher for the prerender manifest.
 *
 * This class is used to match the pathname to the dynamic route.
 */ export class PrerenderManifestMatcher {
    constructor(pathname, prerenderManifest){
        this.matchers = Object.entries(prerenderManifest.dynamicRoutes).filter(([source, route])=>{
            // If the pathname is a fallback source route, or the source route is
            // the same as the pathname, then we should include it in the matchers.
            return route.fallbackSourceRoute === pathname || source === pathname;
        }).map(([source, route])=>({
                source,
                route
            }));
    }
    /**
   * Match the pathname to the dynamic route. If no match is found, an error is
   * thrown.
   *
   * @param pathname - The pathname to match.
   * @returns The dynamic route that matches the pathname.
   */ match(pathname) {
        // Iterate over the matchers. They're already in the correct order of
        // specificity as they were inserted into the prerender manifest that way
        // and iterating over them with Object.entries guarantees that.
        for (const matcher of this.matchers){
            // Lazily create the matcher, this is only done once per matcher.
            if (!matcher.matcher) {
                matcher.matcher = getRouteMatcher(getRouteRegex(matcher.source));
            }
            const match = matcher.matcher(pathname);
            if (match) {
                return matcher.route;
            }
        }
        return null;
    }
}

//# sourceMappingURL=prerender-manifest-matcher.js.map