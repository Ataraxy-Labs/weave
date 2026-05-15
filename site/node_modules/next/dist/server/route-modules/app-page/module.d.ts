import type { AppPageRouteDefinition } from '../../route-definitions/app-page-route-definition';
import type RenderResult from '../../render-result';
import type { RenderOpts } from '../../app-render/types';
import type { NextParsedUrlQuery } from '../../request-meta';
import type { LoaderTree } from '../../lib/app-dir-module';
import type { PrerenderManifest } from '../../../build';
import { renderToHTMLOrFlight, type AppSharedContext } from '../../app-render/app-render';
import { RouteModule, type RouteModuleOptions, type RouteModuleHandleContext } from '../route-module';
import * as vendoredContexts from './vendored/contexts/entrypoints';
import type { BaseNextRequest, BaseNextResponse } from '../../base-http';
import type { ServerComponentsHmrCache } from '../../response-cache';
import type { OpaqueFallbackRouteParams } from '../../request/fallback-params';
import type { DeepReadonly } from '../../../shared/lib/deep-readonly';
/**
 * The AppPageModule is the type of the module exported by the bundled app page
 * module.
 */
export type AppPageModule = typeof import('../../../build/templates/app-page');
type AppPageUserlandModule = {
    /**
     * The tree created in next-app-loader that holds component segments and modules
     */
    loaderTree: LoaderTree;
};
export interface AppPageRouteHandlerContext extends RouteModuleHandleContext {
    page: string;
    query: NextParsedUrlQuery;
    fallbackRouteParams: OpaqueFallbackRouteParams | null;
    renderOpts: RenderOpts;
    serverComponentsHmrCache?: ServerComponentsHmrCache;
    sharedContext: AppSharedContext;
}
export type AppPageRouteModuleOptions = RouteModuleOptions<AppPageRouteDefinition, AppPageUserlandModule>;
export declare class AppPageRouteModule extends RouteModule<AppPageRouteDefinition, AppPageUserlandModule> {
    private matchers;
    match(pathname: string, prerenderManifest: DeepReadonly<PrerenderManifest>): {
        readonly dataRoute: string | null;
        readonly dataRouteRegex: string | null;
        readonly experimentalBypassFor?: readonly ({
            readonly type: "header" | "cookie" | "query";
            readonly key: string;
            readonly value?: string | undefined;
        } | {
            readonly type: "host";
            readonly key?: undefined | undefined;
            readonly value: string;
        })[] | undefined;
        readonly fallback: string | boolean | null;
        readonly fallbackRevalidate: import("../../lib/cache-control").Revalidate | undefined;
        readonly fallbackExpire: number | undefined;
        readonly fallbackHeaders?: {
            readonly [x: string]: string;
        } | undefined;
        readonly fallbackStatus?: number | undefined;
        readonly fallbackRootParams: readonly string[] | undefined;
        readonly fallbackRouteParams: readonly {
            readonly paramName: string;
            readonly paramType: import("../../../shared/lib/app-router-types").DynamicParamTypes;
        }[] | undefined;
        readonly fallbackSourceRoute: string | undefined;
        readonly prefetchDataRoute: string | null | undefined;
        readonly prefetchDataRouteRegex: string | null | undefined;
        readonly routeRegex: string;
        readonly experimentalPPR: boolean | undefined;
        readonly renderingMode: import("../../../build/rendering-mode").RenderingMode | undefined;
        readonly allowHeader: readonly string[];
    } | null;
    render(req: BaseNextRequest, res: BaseNextResponse, context: AppPageRouteHandlerContext): Promise<RenderResult>;
    private pathCouldBeIntercepted;
    getVaryHeader(resolvedPathname: string, interceptionRoutePatterns: RegExp[]): string;
}
declare const vendored: {
    'react-rsc': typeof import("./vendored/rsc/entrypoints") | undefined;
    'react-ssr': typeof import("./vendored/ssr/entrypoints") | undefined;
    contexts: typeof vendoredContexts;
};
export { renderToHTMLOrFlight, vendored };
export default AppPageRouteModule;
