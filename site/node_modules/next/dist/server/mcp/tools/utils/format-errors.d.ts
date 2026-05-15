import type { OverlayState } from '../../../../next-devtools/dev-overlay/shared';
import type { OriginalStackFramesRequest, OriginalStackFramesResponse } from '../../../../next-devtools/server/shared';
type StackFrameResolver = (request: OriginalStackFramesRequest) => Promise<OriginalStackFramesResponse>;
export declare function setStackFrameResolver(fn: StackFrameResolver): void;
export declare function formatErrors(errorsByUrl: Map<string, OverlayState>, nextInstanceErrors?: {
    nextConfig: unknown[];
}): Promise<string>;
export {};
