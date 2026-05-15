import type { NextConfigComplete } from '../../server/config-shared';
export type AnalyzeContext = {
    config: NextConfigComplete;
    distDir: string;
    dir: string;
    noMangling: boolean;
    appDirOnly: boolean;
};
export declare function turbopackAnalyze(analyzeContext: AnalyzeContext): Promise<{
    duration: number;
    shutdownPromise: Promise<void>;
}>;
export declare function waitForShutdown(): Promise<void>;
