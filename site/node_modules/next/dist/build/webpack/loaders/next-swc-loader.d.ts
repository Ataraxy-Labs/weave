import type { NextConfig } from '../../../types';
import { type WebpackLayerName } from '../../../lib/constants';
import { type CompilerNameValues } from '../../../shared/lib/constants';
export interface SWCLoaderOptions {
    rootDir: string;
    isServer: boolean;
    compilerType: CompilerNameValues;
    pagesDir?: string;
    appDir?: string;
    hasReactRefresh: boolean;
    optimizeServerReact?: boolean;
    nextConfig: NextConfig;
    jsConfig: any;
    supportedBrowsers: string[] | undefined;
    swcCacheDir: string;
    serverComponents?: boolean;
    serverReferenceHashSalt: string;
    bundleLayer?: WebpackLayerName;
    esm?: boolean;
    transpilePackages?: string[];
}
export declare function pitch(this: any): void;
export default function swcLoader(this: any, inputSource: string, inputSourceMap: any): void;
export declare const raw = true;
