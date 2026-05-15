import { PluginCreator } from 'postcss';

type PluginOptions = {
    /**
     * The base directory to scan for class candidates.
     *
     * Defaults to the current working directory.
     */
    base?: string;
    /**
     * Optimize and minify the output CSS.
     */
    optimize?: boolean | {
        minify?: boolean;
    };
    /**
     * Enable or disable asset URL rewriting.
     *
     * Defaults to `true`.
     */
    transformAssetUrls?: boolean;
};
declare const _default: PluginCreator<PluginOptions>;

export { type PluginOptions, _default as default };
