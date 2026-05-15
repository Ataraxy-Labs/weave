import { InvariantError } from '../../shared/lib/invariant-error';
import { serializeUseCacheCacheStore, parseUseCacheCacheStore } from './cache-store';
/**
 * Serializes a resume data cache into a JSON string for storage or
 * transmission. Handles 'use cache' values, fetch responses, and encrypted
 * bound args for inline server functions.
 *
 * @param resumeDataCache - The immutable cache to serialize
 * @returns A Promise that resolves to the serialized cache as a JSON string, or
 * 'null' if empty
 */ export async function stringifyResumeDataCache(resumeDataCache, isCacheComponentsEnabled) {
    if (process.env.NEXT_RUNTIME === 'edge') {
        throw Object.defineProperty(new InvariantError('`stringifyResumeDataCache` should not be called in edge runtime.'), "__NEXT_ERROR_CODE", {
            value: "E602",
            enumerable: false,
            configurable: true
        });
    } else {
        if (resumeDataCache.fetch.size === 0 && resumeDataCache.cache.size === 0) {
            return 'null';
        }
        const json = {
            store: {
                fetch: Object.fromEntries(Array.from(resumeDataCache.fetch.entries())),
                cache: Object.fromEntries((await serializeUseCacheCacheStore(resumeDataCache.cache.entries(), isCacheComponentsEnabled)).filter((entry)=>entry !== null)),
                encryptedBoundArgs: Object.fromEntries(Array.from(resumeDataCache.encryptedBoundArgs.entries()))
            }
        };
        // Compress the JSON string using zlib. As the data we already want to
        // decompress is in memory, we use the synchronous deflateSync function.
        const { deflateSync } = require('node:zlib');
        return deflateSync(JSON.stringify(json)).toString('base64');
    }
}
/**
 * Creates a new empty mutable resume data cache for pre-rendering.
 * Initializes fresh Map instances for both the 'use cache' and fetch caches.
 * Used at the start of pre-rendering to begin collecting cached values.
 *
 * @returns A new empty PrerenderResumeDataCache instance
 */ export function createPrerenderResumeDataCache() {
    return {
        cache: new Map(),
        fetch: new Map(),
        encryptedBoundArgs: new Map(),
        decryptedBoundArgs: new Map()
    };
}
export function createRenderResumeDataCache(resumeDataCacheOrPersistedCache, maxPostponedStateSizeBytes) {
    if (process.env.NEXT_RUNTIME === 'edge') {
        throw Object.defineProperty(new InvariantError('`createRenderResumeDataCache` should not be called in edge runtime.'), "__NEXT_ERROR_CODE", {
            value: "E556",
            enumerable: false,
            configurable: true
        });
    } else {
        if (typeof resumeDataCacheOrPersistedCache !== 'string') {
            // If the cache is already a prerender or render cache, we can return it
            // directly. For the former, we're just performing a type change.
            return resumeDataCacheOrPersistedCache;
        }
        if (resumeDataCacheOrPersistedCache === 'null') {
            return {
                cache: new Map(),
                fetch: new Map(),
                encryptedBoundArgs: new Map(),
                decryptedBoundArgs: new Map()
            };
        }
        // This should be a compressed string. Let's decompress it using zlib.
        // As the data we already want to decompress is in memory, we use the
        // synchronous inflateSync function.
        const { inflateSync } = require('node:zlib');
        // Limit decompressed size to prevent zipbomb attacks. This is 5x the
        // configured maxPostponedStateSize, allowing reasonable compression
        // ratios while preventing extreme decompression bombs.
        // Default is 500MB (5x the default 100MB compressed limit).
        const maxDecompressedSize = maxPostponedStateSizeBytes ? maxPostponedStateSizeBytes * 5 : 500 * 1024 * 1024;
        let json;
        try {
            json = JSON.parse(inflateSync(Buffer.from(resumeDataCacheOrPersistedCache, 'base64'), {
                maxOutputLength: maxDecompressedSize
            }).toString('utf-8'));
        } catch (err) {
            if (err instanceof RangeError && err.code === 'ERR_BUFFER_TOO_LARGE') {
                throw Object.defineProperty(new Error(`Decompressed resume data cache exceeded ${maxDecompressedSize} byte limit`), "__NEXT_ERROR_CODE", {
                    value: "E976",
                    enumerable: false,
                    configurable: true
                });
            }
            throw err;
        }
        return {
            cache: parseUseCacheCacheStore(Object.entries(json.store.cache)),
            fetch: new Map(Object.entries(json.store.fetch)),
            encryptedBoundArgs: new Map(Object.entries(json.store.encryptedBoundArgs)),
            decryptedBoundArgs: new Map()
        };
    }
}

//# sourceMappingURL=resume-data-cache.js.map