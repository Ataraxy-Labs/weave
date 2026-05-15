import type { IncomingMessage, ServerResponse } from 'node:http';
import { AppPageRouteModule } from '../../server/route-modules/app-page/module.compiled';
import GlobalError from 'VAR_MODULE_GLOBAL_ERROR';
export { GlobalError };
export declare const __next_app__: {
    require: (id: string | number) => unknown;
    loadChunk: (id: string | number) => Promise<unknown>;
};
export * from '../../server/app-render/entry-base';
export declare const routeModule: AppPageRouteModule;
export declare function handler(req: IncomingMessage, res: ServerResponse, ctx: {
    waitUntil: (prom: Promise<void>) => void;
}): Promise<void | null>;
