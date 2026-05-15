/**
 * https://tc39.es/source-map/#index-map
 */
interface IndexSourceMapSection {
    offset: {
        line: number;
        column: number;
    };
    map: BasicSourceMapPayload;
}
/** https://tc39.es/ecma426/#sec-index-source-map */
interface IndexSourceMap {
    version: number;
    file: string;
    sections: IndexSourceMapSection[];
}
/** https://tc39.es/ecma426/#sec-source-map-format */
export interface BasicSourceMapPayload {
    version: number;
    /** WARNING: `file` is optional. */
    file: string;
    sourceRoot?: string;
    /** WARNING: `sources[number]` can be `null`. */
    sources: Array<string>;
    names: Array<string>;
    mappings: string;
    ignoreList?: number[];
}
export type ModernSourceMapPayload = BasicSourceMapPayload | IndexSourceMap;
export declare function sourceMapIgnoreListsEverything(sourceMap: BasicSourceMapPayload): boolean;
/**
 * Finds the sourcemap payload applicable to a given frame.
 * Equal to the input unless an Index Source Map is used.
 * @param line0 - The line number of the frame, 0-based.
 * @param column0 - The column number of the frame, 0-based.
 */
export declare function findApplicableSourceMapPayload(line0: number, column0: number, payload: ModernSourceMapPayload): BasicSourceMapPayload | undefined;
export declare function filterStackFrameDEV(sourceURL: string, functionName: string, line1: number, column1: number): boolean;
export declare function findSourceMapURLDEV(scriptNameOrSourceURL: string): string | null;
export declare function devirtualizeReactServerURL(sourceURL: string): string;
export declare function ignoreListAnonymousStackFramesIfSandwiched<Frame>(frames: Frame[], isAnonymousFrame: (frame: Frame) => boolean, isIgnoredFrame: (frame: Frame) => boolean, getMethodName: (frame: Frame) => string, 
/** only passes frames for which `isAnonymousFrame` and their method is a native JS method or `isIgnoredFrame` return true */
ignoreFrame: (frame: Frame) => void): void;
export {};
