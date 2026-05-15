import type { SupportedErrorEvent } from '../container/runtime-error/render-error';
import type { OriginalStackFrame } from '../../shared/stack-frame';
export type ReadyRuntimeError = {
    id: number;
    runtime: true;
    error: Error & {
        environmentName?: string;
    };
    frames: readonly OriginalStackFrame[] | (() => Promise<readonly OriginalStackFrame[]>);
    type: 'runtime' | 'console' | 'recoverable';
};
export declare const useFrames: (error: ReadyRuntimeError | null) => readonly OriginalStackFrame[];
export declare function getErrorByType(event: SupportedErrorEvent, isAppDir: boolean): Promise<ReadyRuntimeError>;
