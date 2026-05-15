import HotReloaderWebpack from './hot-reloader-webpack';
import type { RouteDefinition } from '../route-definitions/route-definition';
import type { MultiCompiler } from 'webpack';
/**
 * Rspack Persistent Cache Strategy for Next.js Development
 *
 * Rspack's persistent caching differs from Webpack in how it manages module graphs.
 * While Webpack incrementally updates modules, Rspack operates on complete module
 * graph snapshots for cache restoration.
 *
 * Problem:
 * - Next.js dev server starts with no page modules in the initial entry points
 * - When Rspack restores from persistent cache, it finds no modules and purges
 *   the entire module graph
 * - Later page requests find no cached module information, preventing cache reuse
 *
 * Solution:
 * - Track successfully built page entries after each compilation
 * - Restore these entries on dev server restart to maintain module graph continuity
 * - This ensures previously compiled pages can leverage persistent cache for faster builds
 */
export default class HotReloaderRspack extends HotReloaderWebpack {
    private builtEntriesCachePath?;
    private isClientCacheEnabled;
    private isServerCacheEnabled;
    private isEdgeServerCacheEnabled;
    afterCompile(multiCompiler: MultiCompiler): Promise<void>;
    ensurePage({ page, clientOnly, appPaths, definition, isApp, url, }: {
        page: string;
        clientOnly: boolean;
        appPaths?: ReadonlyArray<string> | null;
        isApp?: boolean;
        definition?: RouteDefinition;
        url?: string;
    }): Promise<void>;
}
