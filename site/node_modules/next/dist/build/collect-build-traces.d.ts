import { Span } from '../trace';
import type { NextConfigComplete } from '../server/config-shared';
import { type BuildTraceContext } from './webpack/plugins/next-trace-entrypoints-plugin';
import type { RoutesUsingEdgeRuntime } from './utils';
export declare const makeIgnoreFn: (root: string, ignores: string[]) => (pathname: string) => boolean;
export declare function collectBuildTraces({ dir, config, distDir, edgeRuntimeRoutes, staticPages, nextBuildSpan, buildTraceContext, outputFileTracingRoot, }: {
    dir: string;
    distDir: string;
    staticPages: string[];
    outputFileTracingRoot: string;
    edgeRuntimeRoutes: RoutesUsingEdgeRuntime;
    nextBuildSpan?: Span;
    config: NextConfigComplete;
    buildTraceContext?: BuildTraceContext;
}): Promise<void>;
