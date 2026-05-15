import type { NextConfigComplete } from '../server/config-shared';
import type { Span } from '../trace';
import type { Telemetry } from '../telemetry/storage';
export declare function runAfterProductionCompile({ config, buildSpan, telemetry, metadata, }: {
    config: NextConfigComplete;
    buildSpan: Span;
    telemetry: Telemetry;
    metadata: {
        projectDir: string;
        distDir: string;
    };
}): Promise<void>;
