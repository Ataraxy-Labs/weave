import type { ServerResponse, IncomingMessage } from 'http';
import type { Telemetry } from '../../telemetry/storage';
import type { Project } from '../../build/swc/types';
interface RestartDevServerMiddlewareConfig {
    telemetry: Telemetry;
    turbopackProject?: Project;
    webpackCacheDirectories?: Set<string>;
}
export declare function getRestartDevServerMiddleware({ telemetry, turbopackProject, webpackCacheDirectories, }: RestartDevServerMiddlewareConfig): (req: IncomingMessage, res: ServerResponse, next: () => void) => Promise<void>;
export {};
