#!/usr/bin/env node
import '../server/lib/cpu-profile';
export type NextAnalyzeOptions = {
    experimentalAnalyze?: boolean;
    profile?: boolean;
    mangling: boolean;
    port: number;
    output: boolean;
    experimentalAppOnly?: boolean;
};
declare const nextAnalyze: (options: NextAnalyzeOptions, directory?: string) => Promise<void>;
export { nextAnalyze };
