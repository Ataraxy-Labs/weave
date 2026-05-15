import { pathToRegexp } from 'next/dist/compiled/path-to-regexp';
import { normalizeRouteRegex } from './load-custom-routes';
import { getRedirectStatus, modifyRouteRegex } from './redirect-status';
export function buildCustomRoute(type, route, restrictedRedirectPaths) {
    const compiled = pathToRegexp(route.source, [], {
        strict: true,
        sensitive: false,
        delimiter: '/'
    });
    // If this is an internal rewrite and it already provides a regex, use it
    // otherwise, normalize the source to a regex.
    let regex;
    if (!route.internal || type !== 'rewrite' || !('regex' in route) || typeof route.regex !== 'string') {
        let source = compiled.source;
        if (!route.internal) {
            source = modifyRouteRegex(source, type === 'redirect' ? restrictedRedirectPaths : undefined);
        }
        regex = normalizeRouteRegex(source);
    } else {
        regex = route.regex;
    }
    if (type !== 'redirect') {
        return {
            ...route,
            regex
        };
    }
    return {
        ...route,
        statusCode: getRedirectStatus(route),
        permanent: undefined,
        regex
    };
}

//# sourceMappingURL=build-custom-route.js.map