import { type WebpackLayerName } from '../lib/constants';
import type { NextConfigComplete } from '../server/config-shared';
interface CompilerAliases {
    [alias: string]: string | string[];
}
export declare function createWebpackAliases({ distDir, isClient, isEdgeServer, dev, config, pagesDir, appDir, dir, reactProductionProfiling, }: {
    distDir: string;
    isClient: boolean;
    isEdgeServer: boolean;
    dev: boolean;
    config: NextConfigComplete;
    pagesDir: string | undefined;
    appDir: string | undefined;
    dir: string;
    reactProductionProfiling: boolean;
}): CompilerAliases;
export declare function createServerOnlyClientOnlyAliases(isServer: boolean): CompilerAliases;
export declare function createNextApiEsmAliases(): Record<string, string>;
export declare function createAppRouterApiAliases(isServerOnlyLayer: boolean): Record<string, string>;
type BundledReactChannel = '' | '-experimental';
export declare function createVendoredReactAliases(bundledReactChannel: BundledReactChannel, { layer, isBrowser, isEdgeServer, reactProductionProfiling, }: {
    layer: WebpackLayerName;
    isBrowser: boolean;
    isEdgeServer: boolean;
    reactProductionProfiling: boolean;
}): CompilerAliases;
export declare function getOptimizedModuleAliases(): CompilerAliases;
export {};
