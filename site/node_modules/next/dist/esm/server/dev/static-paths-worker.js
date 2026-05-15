import '../require-hook';
import '../node-environment';
import { collectSegments } from '../../build/segment-config/app/app-segments';
import { loadComponents } from '../load-components';
import { setHttpClientAndAgentOptions } from '../setup-http-agent-env';
import { isAppPageRouteModule } from '../route-modules/checks';
import { checkIsRoutePPREnabled } from '../lib/experimental/ppr';
import { InvariantError } from '../../shared/lib/invariant-error';
import { collectRootParamKeys } from '../../build/segment-config/app/collect-root-param-keys';
import { buildAppStaticPaths } from '../../build/static-paths/app';
import { buildPagesStaticPaths } from '../../build/static-paths/pages';
import { createIncrementalCache } from '../../export/helpers/create-incremental-cache';
import { parseAppRoute } from '../../shared/lib/router/routes/app';
// we call getStaticPaths in a separate process to ensure
// side-effects aren't relied on in dev that will break
// during a production build
export async function loadStaticPaths({ dir, distDir, pathname, config, httpAgentOptions, locales, defaultLocale, isAppPath, page, isrFlushToDisk, fetchCacheKeyPrefix, cacheMaxMemorySize, requestHeaders, cacheHandler, cacheHandlers, cacheLifeProfiles, nextConfigOutput, buildId, authInterrupts, sriEnabled }) {
    // this needs to be initialized before loadComponents otherwise
    // "use cache" could be missing it's cache handlers
    await createIncrementalCache({
        dir,
        distDir,
        cacheHandler,
        cacheHandlers,
        requestHeaders,
        fetchCacheKeyPrefix,
        flushToDisk: isrFlushToDisk,
        cacheMaxMemorySize
    });
    // update work memory runtime-config
    setHttpClientAndAgentOptions({
        httpAgentOptions
    });
    const components = await loadComponents({
        distDir,
        // In `pages/`, the page is the same as the pathname.
        page: page || pathname,
        isAppPath,
        isDev: true,
        sriEnabled,
        needsManifestsForLegacyReasons: true
    });
    if (isAppPath) {
        const routeModule = components.routeModule;
        const segments = await collectSegments(// We know this is an app page or app route module because we checked
        // above that the page type is 'app'.
        routeModule);
        const route = parseAppRoute(pathname, true);
        if (route.dynamicSegments.length === 0) {
            throw Object.defineProperty(new InvariantError(`Expected a dynamic route, but got a static route: ${pathname}`), "__NEXT_ERROR_CODE", {
                value: "E930",
                enumerable: false,
                configurable: true
            });
        }
        const isRoutePPREnabled = isAppPageRouteModule(routeModule) && checkIsRoutePPREnabled(config.pprConfig);
        const rootParamKeys = collectRootParamKeys(routeModule);
        return buildAppStaticPaths({
            dir,
            page: pathname,
            route,
            cacheComponents: config.cacheComponents,
            segments,
            distDir,
            requestHeaders,
            cacheHandler,
            cacheLifeProfiles,
            isrFlushToDisk,
            fetchCacheKeyPrefix,
            cacheMaxMemorySize,
            ComponentMod: components.ComponentMod,
            nextConfigOutput,
            isRoutePPREnabled,
            buildId,
            authInterrupts,
            rootParamKeys
        });
    } else if (!components.getStaticPaths) {
        // We shouldn't get to this point since the worker should only be called for
        // SSG pages with getStaticPaths.
        throw Object.defineProperty(new InvariantError(`Failed to load page with getStaticPaths for ${pathname}`), "__NEXT_ERROR_CODE", {
            value: "E605",
            enumerable: false,
            configurable: true
        });
    }
    return buildPagesStaticPaths({
        page: pathname,
        getStaticPaths: components.getStaticPaths,
        configFileName: config.configFileName,
        locales,
        defaultLocale
    });
}

//# sourceMappingURL=static-paths-worker.js.map