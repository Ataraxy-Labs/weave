import { McpServer } from 'next/dist/compiled/@modelcontextprotocol/sdk/server/mcp';
import type { HmrMessageSentToBrowser } from '../dev/hot-reloader-types';
import type { NextConfigComplete } from '../config-shared';
export interface McpServerOptions {
    projectPath: string;
    distDir: string;
    nextConfig: NextConfigComplete;
    pagesDir: string | undefined;
    appDir: string | undefined;
    sendHmrMessage: (message: HmrMessageSentToBrowser) => void;
    getActiveConnectionCount: () => number;
    getDevServerUrl: () => string | undefined;
}
export declare const getOrCreateMcpServer: (options: McpServerOptions) => McpServer;
