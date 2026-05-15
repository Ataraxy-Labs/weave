import type { RenderOpts, PreloadCallbacks } from './types';
import type { ActionResult, DynamicParamTypesShort, FlightRouterState, Segment, CacheNodeSeedData } from '../../shared/lib/app-router-types';
import { type WorkStore } from '../app-render/work-async-storage.external';
import type { RequestStore } from '../app-render/work-unit-async-storage.external';
import type { NextParsedUrlQuery } from '../request-meta';
import type { AppPageModule } from '../route-modules/app-page/module';
import type { BaseNextRequest, BaseNextResponse } from '../base-http';
import RenderResult, { type AppPageRenderResultMetadata } from '../render-result';
import { type ImplicitTags } from '../lib/implicit-tags';
import { parseRelativeUrl } from '../../shared/lib/router/utils/parse-relative-url';
import type { ServerComponentsHmrCache } from '../response-cache';
import type { OpaqueFallbackRouteParams } from '../request/fallback-params';
export type GetDynamicParamFromSegment = (segment: string) => DynamicParam | null;
export type DynamicParam = {
    param: string;
    value: string | string[] | null;
    treeSegment: Segment;
    type: DynamicParamTypesShort;
};
export type GenerateFlight = typeof generateDynamicFlightRenderResult;
export type AppSharedContext = {
    buildId: string;
};
export type AppRenderContext = {
    sharedContext: AppSharedContext;
    workStore: WorkStore;
    url: ReturnType<typeof parseRelativeUrl>;
    componentMod: AppPageModule;
    renderOpts: RenderOpts;
    parsedRequestHeaders: ParsedRequestHeaders;
    getDynamicParamFromSegment: GetDynamicParamFromSegment;
    query: NextParsedUrlQuery;
    isPrefetch: boolean;
    isPossibleServerAction: boolean;
    requestTimestamp: number;
    appUsingSizeAdjustment: boolean;
    flightRouterState?: FlightRouterState;
    requestId: string;
    htmlRequestId: string;
    pagePath: string;
    assetPrefix: string;
    isNotFoundPath: boolean;
    nonce: string | undefined;
    res: BaseNextResponse;
    /**
     * For now, the implicit tags are common for the whole route. If we ever start
     * rendering/revalidating segments independently, they need to move to the
     * work unit store.
     */
    implicitTags: ImplicitTags;
};
interface ParsedRequestHeaders {
    /**
     * Router state provided from the client-side router. Used to handle rendering
     * from the common layout down. This value will be undefined if the request is
     * not a client-side navigation request, or if the request is a prefetch
     * request.
     */
    readonly flightRouterState: FlightRouterState | undefined;
    readonly isPrefetchRequest: boolean;
    readonly isRuntimePrefetchRequest: boolean;
    readonly isRouteTreePrefetchRequest: boolean;
    readonly isHmrRefresh: boolean;
    readonly isRSCRequest: boolean;
    readonly nonce: string | undefined;
    readonly previouslyRevalidatedTags: string[];
    readonly requestId: string | undefined;
    readonly htmlRequestId: string | undefined;
}
/**
 * Produces a RenderResult containing the Flight data for the given request. See
 * `generateDynamicRSCPayload` for information on the contents of the render result.
 */
declare function generateDynamicFlightRenderResult(req: BaseNextRequest, ctx: AppRenderContext, requestStore: RequestStore, options?: {
    actionResult: ActionResult;
    skipPageRendering: boolean;
    componentTree?: CacheNodeSeedData;
    preloadCallbacks?: PreloadCallbacks;
    temporaryReferences?: WeakMap<any, string>;
    waitUntil?: Promise<unknown>;
}): Promise<RenderResult>;
export type BinaryStreamOf<T> = ReadableStream<Uint8Array>;
export type AppPageRender = (req: BaseNextRequest, res: BaseNextResponse, pagePath: string, query: NextParsedUrlQuery, fallbackRouteParams: OpaqueFallbackRouteParams | null, renderOpts: RenderOpts, serverComponentsHmrCache: ServerComponentsHmrCache | undefined, sharedContext: AppSharedContext) => Promise<RenderResult<AppPageRenderResultMetadata>>;
export declare const renderToHTMLOrFlight: AppPageRender;
export {};
