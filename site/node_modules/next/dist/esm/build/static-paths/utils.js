import { isAppPageRouteModule } from '../../server/route-modules/checks';
import { parseAppRouteSegment } from '../../shared/lib/router/routes/app';
import { parseLoaderTree } from '../../shared/lib/router/utils/parse-loader-tree';
import { extractPathnameRouteParamSegmentsFromLoaderTree } from './app/extract-pathname-route-param-segments-from-loader-tree';
import { resolveParamValue } from '../../shared/lib/router/utils/resolve-param-value';
/**
 * Encodes a parameter value using the provided encoder.
 *
 * @param value - The value to encode.
 * @param encoder - The encoder to use.
 * @returns The encoded value.
 */ export function encodeParam(value, encoder) {
    let replaceValue;
    if (Array.isArray(value)) {
        replaceValue = value.map(encoder).join('/');
    } else {
        replaceValue = encoder(value);
    }
    return replaceValue;
}
/**
 * Normalizes a pathname to a consistent format.
 *
 * @param pathname - The pathname to normalize.
 * @returns The normalized pathname.
 */ export function normalizePathname(pathname) {
    return pathname.replace(/\\/g, '/').replace(/(?!^)\/$/, '');
}
/**
 * Extracts segments that contribute to the pathname by traversing the loader tree
 * based on the route module type.
 *
 * @param routeModule - The app route module (page or route handler)
 * @param segments - Array of AppSegment objects collected from the route
 * @param page - The target pathname to match against, INCLUDING interception
 *               markers (e.g., "/blog/[slug]", "/(.)photo/[id]")
 * @returns Array of segments with param info that contribute to the pathname
 */ export function extractPathnameRouteParamSegments(routeModule, segments, route) {
    // For AppPageRouteModule, use the loaderTree traversal approach
    if (isAppPageRouteModule(routeModule)) {
        const { pathnameRouteParamSegments } = extractPathnameRouteParamSegmentsFromLoaderTree(routeModule.userland.loaderTree, route);
        return pathnameRouteParamSegments;
    }
    return extractPathnameRouteParamSegmentsFromSegments(segments);
}
export function extractPathnameRouteParamSegmentsFromSegments(segments) {
    // TODO: should we consider what values are already present in the page?
    // For AppRouteRouteModule, filter the segments array to get the route params
    // that contribute to the pathname.
    const result = [];
    for (const segment of segments){
        // Skip segments without param info.
        if (!segment.paramName || !segment.paramType) continue;
        // Collect all the route param keys that contribute to the pathname.
        result.push({
            name: segment.name,
            paramName: segment.paramName,
            paramType: segment.paramType
        });
    }
    return result;
}
/**
 * Resolves all route parameters from the loader tree. This function uses
 * tree-based traversal to correctly handle the hierarchical structure of routes
 * and accurately determine parameter values based on their depth in the tree.
 *
 * This processes both regular route parameters (from the main children route) and
 * parallel route parameters (from slots like @modal, @sidebar).
 *
 * Unlike interpolateParallelRouteParams (which has a complete URL at runtime),
 * this build-time function determines which route params are unknown.
 * The pathname may contain placeholders like [slug], making it incomplete.
 *
 * @param loaderTree - The loader tree structure containing route hierarchy
 * @param params - The current route parameters object (will be mutated)
 * @param route - The current route being processed
 * @param fallbackRouteParams - Array of fallback route parameters (will be mutated)
 */ export function resolveRouteParamsFromTree(loaderTree, params, route, fallbackRouteParams) {
    // Stack-based traversal with depth tracking
    const stack = [
        {
            tree: loaderTree,
            depth: 0
        }
    ];
    while(stack.length > 0){
        const { tree, depth } = stack.pop();
        const { segment, parallelRoutes } = parseLoaderTree(tree);
        const appSegment = parseAppRouteSegment(segment);
        // If this segment is a route parameter, then we should process it if it's
        // not already known and is not already marked as a fallback route param.
        if ((appSegment == null ? void 0 : appSegment.type) === 'dynamic' && !params.hasOwnProperty(appSegment.param.paramName) && !fallbackRouteParams.some((param)=>param.paramName === appSegment.param.paramName)) {
            const { paramName, paramType } = appSegment.param;
            const paramValue = resolveParamValue(paramName, paramType, depth, route, params);
            if (paramValue !== undefined) {
                params[paramName] = paramValue;
            } else if (paramType !== 'optional-catchall') {
                // If we couldn't resolve the param, mark it as a fallback
                fallbackRouteParams.push({
                    paramName,
                    paramType
                });
            }
        }
        // Calculate next depth - increment if this is not a route group and not empty
        let nextDepth = depth;
        if (appSegment && appSegment.type !== 'route-group' && appSegment.type !== 'parallel-route') {
            nextDepth++;
        }
        // Add all parallel routes to the stack for processing.
        for (const parallelRoute of Object.values(parallelRoutes)){
            stack.push({
                tree: parallelRoute,
                depth: nextDepth
            });
        }
    }
}

//# sourceMappingURL=utils.js.map