import type { Project } from '../../../build/swc/types';
import { type MappingContext } from './source-map';
import { type ServerLogEntry } from '../../../next-devtools/shared/forward-logs-shared';
export declare function restoreUndefined(x: any): any;
export declare function stripFormatSpecifiers(args: any[]): any[];
export declare function handleLog(entries: ServerLogEntry[], ctx: MappingContext, distDir: string, config: boolean | {
    logDepth?: number;
    showSourceLocation?: boolean;
}): Promise<void>;
export declare function receiveBrowserLogsWebpack(opts: {
    entries: ServerLogEntry[];
    router: 'app' | 'pages';
    sourceType?: 'server' | 'edge-server';
    clientStats: () => any;
    serverStats: () => any;
    edgeServerStats: () => any;
    rootDirectory: string;
    distDir: string;
    config: boolean | {
        logDepth?: number;
        showSourceLocation?: boolean;
    };
}): Promise<void>;
export declare function receiveBrowserLogsTurbopack(opts: {
    entries: ServerLogEntry[];
    router: 'app' | 'pages';
    sourceType?: 'server' | 'edge-server';
    project: Project;
    projectPath: string;
    distDir: string;
    config: boolean | {
        logDepth?: number;
        showSourceLocation?: boolean;
    };
}): Promise<void>;
export declare function handleClientFileLogs(logs: Array<{
    timestamp: string;
    level: string;
    message: string;
}>): Promise<void>;
