import { type Group } from '../../../shared/lib/router/utils/route-regex';
import type { NextConfigComplete } from '../../config-shared';
interface RouteInfo {
    path: string;
    groups: {
        [groupName: string]: Group;
    };
}
export interface RouteTypesManifest {
    appRoutes: Record<string, RouteInfo>;
    pageRoutes: Record<string, RouteInfo>;
    layoutRoutes: Record<string, RouteInfo & {
        slots: string[];
    }>;
    appRouteHandlerRoutes: Record<string, RouteInfo>;
    /** Map of redirect source => RouteInfo */
    redirectRoutes: Record<string, RouteInfo>;
    /** Map of rewrite source => RouteInfo */
    rewriteRoutes: Record<string, RouteInfo>;
    /** File paths for validation */
    appPagePaths: Set<string>;
    pagesRouterPagePaths: Set<string>;
    layoutPaths: Set<string>;
    appRouteHandlers: Set<string>;
    pageApiRoutes: Set<string>;
    /** Direct mapping from file paths to routes for validation */
    filePathToRoute: Map<string, string>;
}
export declare function convertCustomRouteSource(source: string): string[];
/**
 * Extracts route parameters from a route pattern
 */
export declare function extractRouteParams(route: string): {
    [groupName: string]: Group;
};
/**
 * Creates a route types manifest from processed route data
 * (used for both build and dev)
 */
export declare function createRouteTypesManifest({ dir, pageRoutes, appRoutes, appRouteHandlers, pageApiRoutes, layoutRoutes, slots, redirects, rewrites, validatorFilePath, }: {
    dir: string;
    pageRoutes: Array<{
        route: string;
        filePath: string;
    }>;
    appRoutes: Array<{
        route: string;
        filePath: string;
    }>;
    appRouteHandlers: Array<{
        route: string;
        filePath: string;
    }>;
    pageApiRoutes: Array<{
        route: string;
        filePath: string;
    }>;
    layoutRoutes: Array<{
        route: string;
        filePath: string;
    }>;
    slots: Array<{
        name: string;
        parent: string;
    }>;
    redirects?: NextConfigComplete['redirects'];
    rewrites?: NextConfigComplete['rewrites'];
    validatorFilePath?: string;
}): Promise<RouteTypesManifest>;
export declare function writeRouteTypesManifest(manifest: RouteTypesManifest, filePath: string, config: NextConfigComplete): Promise<void>;
export declare function writeValidatorFile(manifest: RouteTypesManifest, filePath: string): Promise<void>;
export {};
