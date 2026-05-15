/**
 * Atomically write an invalidation marker.
 *
 * Because attempting to delete currently open cache files could cause issues,
 * actual deletion of files is deferred until the next start-up (in
 * `checkFileSystemCacheInvalidationAndCleanup`).
 *
 * In the case that no database is currently open (e.g. via a separate CLI
 * subcommand), you should call `cleanupFileSystemCache` *after* this to eagerly
 * remove the cache files.
 */
export declare function invalidateFileSystemCache(cacheDirectory: string): Promise<void>;
/**
 * Called during startup. See if the cache is in a partially-completed
 * invalidation state. Finds and delete any invalidated cache files.
 */
export declare function checkFileSystemCacheInvalidationAndCleanup(cacheDirectory: string): Promise<void>;
