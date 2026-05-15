import type { ServerResponse } from 'http';
import type { NextConfigRuntime } from '../config-shared';
export declare function isChromeDevtoolsWorkspaceUrl(pathname: string | undefined): boolean;
export declare function handleChromeDevtoolsWorkspaceRequest(response: ServerResponse, opts: {
    dir: string;
}, config: NextConfigRuntime): Promise<void>;
