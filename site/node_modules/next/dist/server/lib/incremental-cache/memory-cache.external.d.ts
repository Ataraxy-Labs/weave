import type { CacheHandlerValue } from '.';
import { LRUCache } from '../lru-cache';
export declare function getMemoryCache(maxMemoryCacheSize: number): LRUCache<CacheHandlerValue>;
