import type { NextConfigComplete } from '../server/config-shared';
import type { Telemetry } from '../telemetry/storage';
import type { Span } from '../trace';
export declare function startTypeChecking({ cacheDir, config, dir, nextBuildSpan, pagesDir, telemetry, appDir, debugBuildPaths, }: {
    cacheDir: string;
    config: NextConfigComplete;
    dir: string;
    nextBuildSpan: Span;
    pagesDir?: string;
    telemetry: Telemetry;
    appDir?: string;
    debugBuildPaths?: {
        app?: string[];
        pages?: string[];
    };
}): Promise<void>;
