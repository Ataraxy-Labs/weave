import { type ModernSourceMapPayload } from './lib/source-maps';
type FindSourceMapPayload = (sourceURL: string) => ModernSourceMapPayload | undefined;
export declare function setBundlerFindSourceMapImplementation(findSourceMapImplementation: FindSourceMapPayload): void;
export declare function patchErrorInspectNodeJS(errorConstructor: ErrorConstructor): void;
export declare function patchErrorInspectEdgeLite(errorConstructor: ErrorConstructor): void;
export {};
