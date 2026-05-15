import { webpack, sources } from 'next/dist/compiled/webpack/webpack';
type CssMinimizerPluginOptions = {
    postcssOptions: {
        map: false | {
            prev?: string | false;
            inline: boolean;
            annotation: boolean;
        };
    };
};
export declare class CssMinimizerPlugin {
    __next_css_remove: boolean;
    private options;
    constructor(options: CssMinimizerPluginOptions);
    optimizeAsset(file: string, asset: any): Promise<sources.SourceMapSource | sources.RawSource>;
    apply(compiler: webpack.Compiler): void;
}
export {};
