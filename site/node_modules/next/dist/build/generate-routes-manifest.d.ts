import type { NextConfigComplete } from '../server/config-shared';
import type { CustomRoutes } from '../lib/load-custom-routes';
import type { DynamicManifestRoute } from './utils';
import type { RoutesManifest } from './index';
export interface GenerateRoutesManifestOptions {
    pageKeys: {
        pages: string[];
        app?: string[];
    };
    config: NextConfigComplete;
    redirects: CustomRoutes['redirects'];
    headers: CustomRoutes['headers'];
    rewrites: CustomRoutes['rewrites'];
    restrictedRedirectPaths: string[];
    isAppPPREnabled: boolean;
    appType: 'pages' | 'app' | 'hybrid';
}
export interface GenerateRoutesManifestResult {
    routesManifest: RoutesManifest;
    dynamicRoutes: Array<DynamicManifestRoute>;
    sourcePages: Map<string, string>;
}
/**
 * Generates the routes manifest from the given page keys and configuration.
 * This function extracts the route manifest generation logic to be reusable
 * across different build contexts (webpack build, turbopack build, analyze, etc.)
 */
export declare function generateRoutesManifest(options: GenerateRoutesManifestOptions): GenerateRoutesManifestResult;
