import type { NextBabelLoaderOptions, NextJsLoaderContext } from './types';
import { type SourceMap, type BabelLoaderTransformOptions } from './util';
/**
 * An internal (non-exported) type used by babel.
 */
export type ResolvedBabelConfig = {
    options: BabelLoaderTransformOptions;
    passes: BabelPluginPasses;
    externalDependencies: ReadonlyArray<string>;
};
export type BabelPlugin = unknown;
export type BabelPluginPassList = ReadonlyArray<BabelPlugin>;
export type BabelPluginPasses = ReadonlyArray<BabelPluginPassList>;
export default function getConfig(ctx: NextJsLoaderContext, { source, target, loaderOptions, filename, inputSourceMap, }: {
    source: string;
    loaderOptions: NextBabelLoaderOptions;
    target: string;
    filename: string;
    inputSourceMap?: SourceMap | undefined;
}): Promise<ResolvedBabelConfig | null>;
