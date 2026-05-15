import type { webpack } from 'next/dist/compiled/webpack/webpack';
import type ws from 'next/dist/compiled/ws';
import type { DevToolsConfig } from '../../next-devtools/dev-overlay/shared';
import type { VersionInfo } from './parse-version-info';
import type { HmrMessageSentToBrowser } from './hot-reloader-types';
import type { NextConfigComplete } from '../config-shared';
export declare class WebpackHotMiddleware {
    private versionInfo;
    private devtoolsFrontendUrl;
    private config;
    private devToolsConfig;
    private clientsWithoutHtmlRequestId;
    private clientsByHtmlRequestId;
    private closed;
    private clientLatestStats;
    private middlewareLatestStats;
    private serverLatestStats;
    constructor(compilers: webpack.Compiler[], versionInfo: VersionInfo, devtoolsFrontendUrl: string | undefined, config: NextConfigComplete, devToolsConfig: DevToolsConfig);
    onClientInvalid: () => void;
    onClientDone: (statsResult: webpack.Stats) => void;
    onServerInvalid: () => void;
    onServerDone: (statsResult: webpack.Stats) => void;
    onEdgeServerInvalid: () => void;
    onEdgeServerDone: (statsResult: webpack.Stats) => void;
    updateDevToolsConfig(newConfig: DevToolsConfig): void;
    /**
     * To sync we use the most recent stats but also we append middleware
     * errors. This is because it is possible that middleware fails to compile
     * and we still want to show the client overlay with the error while
     * the error page should be rendered just fine.
     */
    onHMR: (client: ws, htmlRequestId: string | null) => void;
    publishStats: (statsResult: webpack.Stats) => void;
    getClient: (htmlRequestId: string) => ws | undefined;
    publishToClient: (client: ws, message: HmrMessageSentToBrowser) => void;
    publish: (message: HmrMessageSentToBrowser) => void;
    publishToLegacyClients: (message: HmrMessageSentToBrowser) => void;
    close: () => void;
    deleteClient: (client: ws, htmlRequestId: string | null) => void;
    hasClients: () => boolean;
    getClientCount: () => number;
}
