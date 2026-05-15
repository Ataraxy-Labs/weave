import type { OriginalStackFrameResponse, StackFrame } from '../server/shared';
export type { StackFrame };
interface ResolvedOriginalStackFrame extends OriginalStackFrameResponse {
    error: false;
    reason: null;
    external: boolean;
    ignored: boolean;
    sourceStackFrame: StackFrame;
}
interface RejectedOriginalStackFrame extends OriginalStackFrameResponse {
    error: true;
    reason: string;
    external: boolean;
    ignored: boolean;
    sourceStackFrame: StackFrame;
}
export type OriginalStackFrame = ResolvedOriginalStackFrame | RejectedOriginalStackFrame;
export declare function getOriginalStackFrames(frames: readonly StackFrame[], type: 'server' | 'edge-server' | null, isAppDir: boolean): Promise<readonly OriginalStackFrame[]>;
export declare function getFrameSource(frame: StackFrame): string;
