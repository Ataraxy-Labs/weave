import type { RouteModule } from '../../../server/route-modules/route-module';
/**
 * Collects the segments for a given route module.
 *
 * @param components the loaded components
 * @returns the segments for the route module
 */
export declare function collectRootParamKeys(routeModule: RouteModule): readonly string[];
