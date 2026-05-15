import type { IncomingMessage, ServerResponse } from 'http';
import type { DevToolsConfig } from '../dev-overlay/shared';
export declare function devToolsConfigMiddleware({ distDir, sendUpdateSignal, }: {
    distDir: string;
    sendUpdateSignal: (data: DevToolsConfig) => void;
}): (req: IncomingMessage, res: ServerResponse, next: () => void) => Promise<void>;
export declare function getDevToolsConfig(distDir: string): Promise<DevToolsConfig>;
