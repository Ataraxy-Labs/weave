import type { FlightDataPath, FlightRouterState, HeadData } from '../../shared/lib/app-router-types';
import type { PreloadCallbacks } from './types';
import type { LoaderTree } from '../lib/app-dir-module';
import type { AppRenderContext } from './app-render';
/**
 * Use router state to decide at what common layout to render the page.
 * This can either be the common layout between two pages or a specific place to start rendering from using the "refetch" marker in the tree.
 */
export declare function walkTreeWithFlightRouterState({ loaderTreeToFilter, parentParams, flightRouterState, parentIsInsideSharedLayout, rscHead, injectedCSS, injectedJS, injectedFontPreloadTags, rootLayoutIncluded, ctx, preloadCallbacks, MetadataOutlet, }: {
    loaderTreeToFilter: LoaderTree;
    parentParams: {
        [key: string]: string | string[];
    };
    flightRouterState?: FlightRouterState;
    rscHead: HeadData;
    parentIsInsideSharedLayout?: boolean;
    injectedCSS: Set<string>;
    injectedJS: Set<string>;
    injectedFontPreloadTags: Set<string>;
    rootLayoutIncluded: boolean;
    ctx: AppRenderContext;
    preloadCallbacks: PreloadCallbacks;
    MetadataOutlet: React.ComponentType;
}): Promise<FlightDataPath[]>;
