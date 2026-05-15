import type { TransformOptions } from 'next/dist/compiled/babel/core';
export declare function consumeIterator(iter: Iterator<any>): any;
/**
 * Source map standard format as to revision 3.
 *
 * `TransformOptions` uses this type, but doesn't export it separately
 */
export type SourceMap = NonNullable<TransformOptions['inputSourceMap']>;
/**
 * An extension of the normal babel configuration, with extra `babel-loader`-specific fields that transforms can read.
 *
 * See: https://github.com/babel/babel-loader/blob/main/src/injectCaller.js
 */
export type BabelLoaderTransformOptions = TransformOptions & {
    target?: string;
};
