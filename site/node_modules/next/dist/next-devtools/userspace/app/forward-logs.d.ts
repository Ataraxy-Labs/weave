import { type ClientLogEntry } from '../../shared/forward-logs-shared';
export declare const logQueue: {
    entries: Array<ClientLogEntry>;
    onSocketReady: (socket: WebSocket) => void;
    flushScheduled: boolean;
    socket: WebSocket | null;
    cancelFlush: (() => void) | null;
    sourceType?: 'server' | 'edge-server';
    router: 'app' | 'pages' | null;
    scheduleLogSend: (entry: ClientLogEntry) => void;
};
export declare const forwardErrorLog: (args: any[]) => void;
export declare function logUnhandledRejection(reason: unknown): void;
export declare function forwardUnhandledError(error: Error): void;
export declare const initializeDebugLogForwarding: (router: "app" | "pages") => void;
