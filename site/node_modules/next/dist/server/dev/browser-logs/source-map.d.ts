import type { Project } from '../../../build/swc/types';
import { type StackFrame } from '../../lib/parse-stack';
type WebpackMappingContext = {
    bundler: 'webpack';
    isServer: boolean;
    isEdgeServer: boolean;
    isAppDirectory: boolean;
    clientStats: () => any;
    serverStats: () => any;
    edgeServerStats: () => any;
    rootDirectory: string;
};
type TurbopackMappingContext = {
    bundler: 'turbopack';
    isServer: boolean;
    isEdgeServer: boolean;
    isAppDirectory: boolean;
    project: Project;
    projectPath: string;
};
export type MappingContext = WebpackMappingContext | TurbopackMappingContext;
export declare function mapFramesUsingBundler(frames: StackFrame[], ctx: MappingContext): Promise<import("../../../next-devtools/server/shared").OriginalStackFramesResponse>;
export declare function getSourceMappedStackFrames(stackTrace: string, ctx: MappingContext, distDir: string, ignore?: boolean): Promise<{
    kind: "stack";
    stack: string;
    frameCode?: undefined;
    frames?: undefined;
} | {
    kind: "all-ignored";
    stack?: undefined;
    frameCode?: undefined;
    frames?: undefined;
} | {
    kind: "with-frame-code";
    frameCode: string;
    stack: string;
    frames: ({
        kind: "rejected";
        frameText: string;
        codeFrame: null;
    } | {
        kind: "success";
        frameText: string;
        codeFrame: string | null;
    })[];
} | {
    kind: "mapped-stack";
    stack: string;
    frames: ({
        kind: "rejected";
        frameText: string;
        codeFrame: null;
    } | {
        kind: "success";
        frameText: string;
        codeFrame: string | null;
    })[];
    frameCode?: undefined;
}>;
export declare const withLocation: ({ original, stack, }: {
    original: Array<any>;
    stack: string | null;
}, ctx: MappingContext, distDir: string, config: boolean | {
    logDepth?: number;
    showSourceLocation?: boolean;
}) => Promise<any[]>;
export declare const getConsoleLocation: (mapped: Awaited<ReturnType<typeof getSourceMappedStackFrames>>) => string | null;
export {};
