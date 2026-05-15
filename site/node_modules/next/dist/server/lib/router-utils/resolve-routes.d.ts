import type { FsOutput } from './filesystem';
import type { IncomingMessage, ServerResponse } from 'http';
import type { NextConfigRuntime } from '../../config-shared';
import type { RenderServer, initialize } from '../router-server';
import type { UnwrapPromise } from '../../../lib/coalesced-function';
import type { NextUrlWithParsedQuery } from '../../request-meta';
export declare function getResolveRoutes(fsChecker: UnwrapPromise<ReturnType<typeof import('./filesystem').setupFsCheck>>, config: NextConfigRuntime, opts: Parameters<typeof initialize>[0], renderServer: RenderServer, renderServerOpts: Parameters<RenderServer['initialize']>[0], ensureMiddleware?: (url?: string) => Promise<void>): ({ req, res, isUpgradeReq, invokedOutputs, }: {
    req: IncomingMessage;
    res: ServerResponse;
    isUpgradeReq: boolean;
    signal: AbortSignal;
    invokedOutputs?: Set<string>;
}) => Promise<{
    finished: boolean;
    statusCode?: number;
    bodyStream?: ReadableStream | null;
    resHeaders: Record<string, string | string[]> | null;
    parsedUrl: NextUrlWithParsedQuery;
    matchedOutput?: FsOutput | null;
}>;
