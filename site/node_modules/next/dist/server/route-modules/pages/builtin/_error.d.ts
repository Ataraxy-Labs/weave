import PagesRouteModule from '../module';
export declare const routeModule: PagesRouteModule;
export declare const handler: (req: import("http").IncomingMessage, res: import("http").ServerResponse, ctx: {
    waitUntil: (prom: Promise<void>) => void;
}) => Promise<void>;
