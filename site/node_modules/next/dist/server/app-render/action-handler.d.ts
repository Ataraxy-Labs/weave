import type { IncomingHttpHeaders } from 'node:http';
import type { SizeLimit } from '../../types';
import type { RequestStore } from '../app-render/work-unit-async-storage.external';
import type { AppRenderContext, GenerateFlight } from './app-render';
import type { AppPageModule } from '../route-modules/app-page/module';
import type { BaseNextRequest, BaseNextResponse } from '../base-http';
import RenderResult, { type AppPageRenderResultMetadata } from '../render-result';
import type { WorkStore } from '../app-render/work-async-storage.external';
declare const enum HostType {
    XForwardedHost = "x-forwarded-host",
    Host = "host"
}
export declare function parseHostHeader(headers: IncomingHttpHeaders, originDomain?: string): {
    type: HostType;
    value: string;
} | undefined;
type ServerActionsConfig = {
    bodySizeLimit?: SizeLimit;
    allowedOrigins?: string[];
};
type HandleActionResult = {
    /** An MPA action threw notFound(), and we need to render the appropriate HTML */
    type: 'not-found';
} | {
    type: 'done';
    result: RenderResult | undefined;
    formState?: any;
}
/** The request turned out not to be a server action. */
 | null;
export declare function handleAction({ req, res, ComponentMod, generateFlight, workStore, requestStore, serverActions, ctx, metadata, }: {
    req: BaseNextRequest;
    res: BaseNextResponse;
    ComponentMod: AppPageModule;
    generateFlight: GenerateFlight;
    workStore: WorkStore;
    requestStore: RequestStore;
    serverActions?: ServerActionsConfig;
    ctx: AppRenderContext;
    metadata: AppPageRenderResultMetadata;
}): Promise<HandleActionResult>;
export {};
