import type { TraceEvent } from '../types';
declare function reportToJsonBuild(event: TraceEvent): void;
declare const _default: {
    flushAll: (opts?: {
        end: boolean;
    }) => Promise<void | undefined> | undefined;
    report: typeof reportToJsonBuild;
};
export default _default;
