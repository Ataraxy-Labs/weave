import type { CacheLife } from '../../use-cache/cache-life';
/**
 * Generates TypeScript type definitions for custom cacheLife profiles.
 * This creates overloaded function signatures for the cacheLife() function
 * that provide autocomplete and documentation for each profile.
 */
export declare function generateCacheLifeTypes(cacheLife: {
    [profile: string]: CacheLife;
}): string;
/**
 * Writes cache-life type definitions to a file if cacheLifeConfig exists.
 * This is used by both the CLI (next type-gen) and dev server to generate
 * cache-life.d.ts in the types directory.
 */
export declare function writeCacheLifeTypes(cacheLifeConfig: undefined | {
    [profile: string]: CacheLife;
}, filePath: string): void;
