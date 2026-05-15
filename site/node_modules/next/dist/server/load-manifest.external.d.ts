import type { DeepReadonly } from '../shared/lib/deep-readonly';
/**
 * Load a manifest file from the file system. Optionally cache the manifest in
 * memory to avoid reading the file multiple times using the provided cache or
 * defaulting to a shared module cache. The manifest is frozen to prevent
 * modifications if it is cached.
 *
 * @param path the path to the manifest file
 * @param shouldCache whether to cache the manifest in memory
 * @param cache the cache to use for storing the manifest
 * @returns the manifest object
 */
export declare function loadManifest<T extends object>(path: string, shouldCache: false): T;
export declare function loadManifest<T extends object>(path: string, shouldCache?: boolean, cache?: Map<string, unknown>, skipParse?: boolean): DeepReadonly<T>;
export declare function loadManifest<T extends object>(path: string, shouldCache?: true, cache?: Map<string, unknown>, skipParse?: boolean): DeepReadonly<T>;
export declare function evalManifest<T extends object>(path: string, shouldCache: false): T;
export declare function evalManifest<T extends object>(path: string, shouldCache?: boolean, cache?: Map<string, unknown>): DeepReadonly<T>;
export declare function evalManifest<T extends object>(path: string, shouldCache?: true, cache?: Map<string, unknown>): DeepReadonly<T>;
export declare function loadManifestFromRelativePath<T extends object>({ projectDir, distDir, manifest, shouldCache, cache, skipParse, handleMissing, useEval, }: {
    projectDir: string;
    distDir: string;
    manifest: string;
    shouldCache?: boolean;
    cache?: Map<string, unknown>;
    skipParse?: boolean;
    handleMissing?: boolean;
    useEval?: boolean;
}): DeepReadonly<T>;
export declare function clearManifestCache(path: string, cache?: Map<string, unknown>): boolean;
