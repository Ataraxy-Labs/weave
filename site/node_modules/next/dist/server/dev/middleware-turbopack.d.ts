import type { IncomingMessage, ServerResponse } from 'http';
import { type OriginalStackFramesResponse, type StackFrame } from '../../next-devtools/server/shared';
import type { Project } from '../../build/swc/types';
export declare function getOverlayMiddleware({ project, projectPath, isSrcDir, }: {
    project: Project;
    projectPath: string;
    isSrcDir: boolean;
}): (req: IncomingMessage, res: ServerResponse, next: () => void) => Promise<void>;
export declare function getSourceMapMiddleware(project: Project): (req: IncomingMessage, res: ServerResponse, next: () => void) => Promise<void>;
export declare function getOriginalStackFrames({ project, projectPath, frames, isServer, isEdgeServer, isAppDirectory, }: {
    project: Project;
    projectPath: string;
    frames: readonly StackFrame[];
    isServer: boolean;
    isEdgeServer: boolean;
    isAppDirectory: boolean;
}): Promise<OriginalStackFramesResponse>;
