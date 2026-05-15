import type { PluginItem } from 'next/dist/compiled/babel/core';
type StyledJsxPlugin = [string, any] | string;
type StyledJsxBabelOptions = {
    plugins?: StyledJsxPlugin[];
    styleModule?: string;
    'babel-test'?: boolean;
} | undefined;
type NextBabelPresetOptions = {
    'preset-env'?: any;
    'preset-react'?: any;
    'class-properties'?: any;
    'transform-runtime'?: any;
    'styled-jsx'?: StyledJsxBabelOptions;
    /**
     * `syntax-typescript` is a subset of `preset-typescript`.
     *
     * - When babel is used in "standalone" mode (e.g. alongside SWC when using
     *   react-compiler or turbopack) we'll prefer the options in
     *   'syntax-typescript`, and fall back to `preset-typescript`.
     *
     * - When babel is used in "default" mode (e.g. with a babel config in
     *   webpack) we'll prefer the options in `preset-typescript`, and fall back
     *   to `syntax-typescript`.
     */
    'preset-typescript'?: any;
    'syntax-typescript'?: {
        disallowAmbiguousJSXLike?: boolean;
        dts?: boolean;
        isTSX?: boolean;
        allExtensions?: boolean;
        ignoreExtensions?: boolean;
    };
};
type BabelPreset = {
    presets?: PluginItem[] | null;
    plugins?: PluginItem[] | null;
    sourceType?: 'script' | 'module' | 'unambiguous';
    overrides?: Array<{
        test: RegExp;
    } & Omit<BabelPreset, 'overrides'>>;
};
declare const _default: (api: any, options?: NextBabelPresetOptions) => BabelPreset;
export default _default;
