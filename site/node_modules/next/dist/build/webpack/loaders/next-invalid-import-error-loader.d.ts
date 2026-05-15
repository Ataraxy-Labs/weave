import type webpack from 'webpack';
export type InvalidImportLoaderOpts = {
    message: string;
};
declare const nextInvalidImportErrorLoader: webpack.LoaderDefinitionFunction<InvalidImportLoaderOpts>;
export default nextInvalidImportErrorLoader;
