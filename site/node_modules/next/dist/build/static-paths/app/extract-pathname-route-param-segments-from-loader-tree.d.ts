import type { LoaderTree } from '../../../server/lib/app-dir-module';
import type { Params } from '../../../server/request/params';
import type { DynamicParamTypes } from '../../../shared/lib/app-router-types';
import { type NormalizedAppRoute } from '../../../shared/lib/router/routes/app';
/**
 * Extracts pathname route param segments from a loader tree and resolves
 * parameter values from static segments in the route.
 *
 * @param loaderTree - The loader tree structure containing route hierarchy
 * @param route - The target route to match against
 * @returns Object containing pathname route param segments and resolved params
 */
export declare function extractPathnameRouteParamSegmentsFromLoaderTree(loaderTree: LoaderTree, route: NormalizedAppRoute): {
    pathnameRouteParamSegments: Array<{
        readonly name: string;
        readonly paramName: string;
        readonly paramType: DynamicParamTypes;
    }>;
    params: Params;
};
