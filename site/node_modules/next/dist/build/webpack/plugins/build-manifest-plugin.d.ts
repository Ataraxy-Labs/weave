import type { BloomFilter } from '../../../shared/lib/bloom-filter';
import type { CustomRoutes } from '../../../lib/load-custom-routes';
import { webpack } from 'next/dist/compiled/webpack/webpack';
import type { BuildManifest } from '../../../server/get-page-files';
export declare function generateClientManifest(assetMap: BuildManifest, rewrites: CustomRoutes['rewrites'], clientRouterFilters?: {
    staticFilter: ReturnType<BloomFilter['export']>;
    dynamicFilter: ReturnType<BloomFilter['export']>;
}, compiler?: any, compilation?: any): string | undefined;
export declare function getEntrypointFiles(entrypoint: any): string[];
export default class BuildManifestPlugin {
    private buildId;
    private rewrites;
    private isDevFallback;
    private appDirEnabled;
    private clientRouterFilters?;
    constructor(options: {
        buildId: string;
        rewrites: CustomRoutes['rewrites'];
        isDevFallback?: boolean;
        appDirEnabled: boolean;
        clientRouterFilters?: Parameters<typeof generateClientManifest>[2];
    });
    createAssets(compiler: any, compilation: any): void;
    apply(compiler: webpack.Compiler): void;
}
