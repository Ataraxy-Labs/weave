#!/usr/bin/env node
import '../server/lib/cpu-profile';
export type NextBuildOptions = {
    experimentalAnalyze?: boolean;
    debug?: boolean;
    debugPrerender?: boolean;
    profile?: boolean;
    mangling: boolean;
    turbo?: boolean;
    turbopack?: boolean;
    webpack?: boolean;
    experimentalDebugMemoryUsage: boolean;
    experimentalAppOnly?: boolean;
    experimentalTurbo?: boolean;
    experimentalBuildMode: 'default' | 'compile' | 'generate' | 'generate-env';
    experimentalUploadTrace?: string;
    experimentalNextConfigStripTypes?: boolean;
    debugBuildPaths?: string;
};
declare const nextBuild: (options: NextBuildOptions, directory?: string) => Promise<void>;
export { nextBuild };
