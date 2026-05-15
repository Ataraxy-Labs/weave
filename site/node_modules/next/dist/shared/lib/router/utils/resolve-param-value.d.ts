import type { Params } from '../../../../server/request/params';
import type { DynamicParamTypes } from '../../app-router-types';
import type { NormalizedAppRoute } from '../routes/app';
/**
 * Resolves a route parameter value from the route segments at the given depth.
 * This shared logic is used by both extractPathnameRouteParamSegmentsFromLoaderTree
 * and resolveRouteParamsFromTree.
 *
 * @param paramName - The parameter name to resolve
 * @param paramType - The parameter type (dynamic, catchall, etc.)
 * @param depth - The current depth in the route tree
 * @param route - The normalized route containing segments
 * @param params - The current params object (used to resolve embedded param references)
 * @param options - Configuration options
 * @returns The resolved parameter value, or undefined if it cannot be resolved
 */
export declare function resolveParamValue(paramName: string, paramType: DynamicParamTypes, depth: number, route: NormalizedAppRoute, params: Params): string | string[] | undefined;
