import type { TraceEvent } from '../types';
export declare function batcher(reportEvents: (evts: TraceEvent[]) => Promise<void>): {
    flushAll: () => Promise<void>;
    report: (event: TraceEvent) => void;
};
declare function reportToJson(event: TraceEvent): void;
declare const _default: {
    flushAll: (opts?: {
        end: boolean;
    }) => Promise<void | undefined> | undefined;
    report: typeof reportToJson;
};
export default _default;
