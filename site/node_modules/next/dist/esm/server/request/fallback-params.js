import { resolveRouteParamsFromTree } from '../../build/static-paths/utils';
import { dynamicParamTypes } from '../app-render/get-short-dynamic-param-type';
import { parseAppRoute } from '../../shared/lib/router/routes/app';
import { extractPathnameRouteParamSegmentsFromLoaderTree } from '../../build/static-paths/app/extract-pathname-route-param-segments-from-loader-tree';
/**
 * Creates an opaque fallback route params object from the fallback route params.
 *
 * @param fallbackRouteParams the fallback route params
 * @returns the opaque fallback route params
 */ export function createOpaqueFallbackRouteParams(fallbackRouteParams) {
    // If there are no fallback route params, we can return early.
    if (fallbackRouteParams.length === 0) return null;
    // As we're creating unique keys for each of the dynamic route params, we only
    // need to generate a unique ID once per request because each of the keys will
    // be also be unique.
    const uniqueID = Math.random().toString(16).slice(2);
    const keys = new Map();
    // Generate a unique key for the fallback route param, if this key is found
    // in the static output, it represents a bug in cache components.
    for (const { paramName, paramType } of fallbackRouteParams){
        keys.set(paramName, [
            `%%drp:${paramName}:${uniqueID}%%`,
            dynamicParamTypes[paramType]
        ]);
    }
    return keys;
}
/**
 * Gets the fallback route params for a given page. This is an expensive
 * operation because it requires parsing the loader tree to extract the fallback
 * route params.
 *
 * @param page the page
 * @param routeModule the route module
 * @returns the opaque fallback route params
 */ export function getFallbackRouteParams(page, routeModule) {
    const route = parseAppRoute(page, true);
    // Extract the pathname-contributing segments from the loader tree. This
    // mirrors the logic in buildAppStaticPaths where we determine which segments
    // actually contribute to the pathname.
    const { pathnameRouteParamSegments, params } = extractPathnameRouteParamSegmentsFromLoaderTree(routeModule.userland.loaderTree, route);
    // Create fallback route params for the pathname segments.
    const fallbackRouteParams = pathnameRouteParamSegments.map(({ paramName, paramType })=>({
            paramName,
            paramType
        }));
    // Resolve route params from the loader tree. This mutates the
    // fallbackRouteParams array to add any route params that are
    // unknown at request time.
    //
    // The page parameter contains placeholders like [slug], which helps
    // resolveRouteParamsFromTree determine which params are unknown.
    resolveRouteParamsFromTree(routeModule.userland.loaderTree, params, route, fallbackRouteParams // Will be mutated to add route params
    );
    // Convert the fallback route params to an opaque format that can be safely
    // used in the postponed state without exposing implementation details.
    return createOpaqueFallbackRouteParams(fallbackRouteParams);
}

//# sourceMappingURL=fallback-params.js.map