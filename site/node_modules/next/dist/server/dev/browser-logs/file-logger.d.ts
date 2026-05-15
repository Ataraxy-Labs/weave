export interface LogEntry {
    timestamp: string;
    source: 'Server' | 'Browser';
    level: string;
    message: string;
}
export declare class FileLogger {
    private logFilePath;
    private isInitialized;
    private logQueue;
    private flushTimer;
    private mcpServerEnabled;
    initialize(distDir: string, mcpServerEnabled: boolean): void;
    private formatTimestamp;
    private formatLogEntry;
    private scheduleFlush;
    getLogQueue(): string[];
    private flush;
    private enqueueLog;
    log(source: 'Server' | 'Browser', level: string, message: string): void;
    logServer(level: string, message: string): void;
    logBrowser(level: string, message: string): void;
    forceFlush(): void;
    destroy(): void;
}
export declare function getFileLogger(): FileLogger;
export declare function test__resetFileLogger(): void;
