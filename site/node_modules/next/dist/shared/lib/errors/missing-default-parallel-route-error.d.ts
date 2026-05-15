import { UsageError } from './usage-error';
export declare class MissingDefaultParallelRouteError extends UsageError {
    constructor(fullSegmentPath: string, slotName: string);
}
