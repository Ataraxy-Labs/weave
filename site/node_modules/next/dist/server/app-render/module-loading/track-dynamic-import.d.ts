/**
 * in CacheComponents, `import(...)` will be transformed into `trackDynamicImport(import(...))`.
 * A dynamic import is essentially a cached async function, except it's cached by the module system.
 *
 * The promises are tracked globally regardless of if the `import()` happens inside a render or outside of it.
 * When rendering, we can make the `cacheSignal` wait for all pending promises via `trackPendingModules`.
 * */
export declare function trackDynamicImport<TExports extends Record<string, any>>(modulePromise: Promise<TExports>): Promise<TExports>;
