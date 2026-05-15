import type { webpack } from 'next/dist/compiled/webpack/webpack';
export type RootParamsLoaderOpts = {
    appDir: string;
    pageExtensions: string[];
};
declare const rootParamsLoader: webpack.LoaderDefinitionFunction<RootParamsLoaderOpts>;
export default rootParamsLoader;
