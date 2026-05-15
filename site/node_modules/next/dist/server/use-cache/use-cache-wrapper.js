"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "cache", {
    enumerable: true,
    get: function() {
        return cache;
    }
});
const _server = require("react-server-dom-webpack/server");
const _client = require("react-server-dom-webpack/client");
const _static = require("react-server-dom-webpack/static");
const _workasyncstorageexternal = require("../app-render/work-async-storage.external");
const _workunitasyncstorageexternal = require("../app-render/work-unit-async-storage.external");
const _dynamicrenderingutils = require("../dynamic-rendering-utils");
const _manifestssingleton = require("../app-render/manifests-singleton");
const _encryption = require("../app-render/encryption");
const _invarianterror = require("../../shared/lib/invariant-error");
const _createerrorhandler = require("../app-render/create-error-handler");
const _constants = require("./constants");
const _handlers = require("./handlers");
const _usecacheerrors = require("./use-cache-errors");
const _dynamicrendering = require("../app-render/dynamic-rendering");
const _searchparams = require("../request/search-params");
const _lazyresult = require("../lib/lazy-result");
const _dynamicaccessasyncstorageexternal = require("../app-render/dynamic-access-async-storage.external");
const _stagedrendering = require("../app-render/staged-rendering");
const _log = /*#__PURE__*/ _interop_require_wildcard(require("../../build/output/log"));
function _getRequireWildcardCache(nodeInterop) {
    if (typeof WeakMap !== "function") return null;
    var cacheBabelInterop = new WeakMap();
    var cacheNodeInterop = new WeakMap();
    return (_getRequireWildcardCache = function(nodeInterop) {
        return nodeInterop ? cacheNodeInterop : cacheBabelInterop;
    })(nodeInterop);
}
function _interop_require_wildcard(obj, nodeInterop) {
    if (!nodeInterop && obj && obj.__esModule) {
        return obj;
    }
    if (obj === null || typeof obj !== "object" && typeof obj !== "function") {
        return {
            default: obj
        };
    }
    var cache = _getRequireWildcardCache(nodeInterop);
    if (cache && cache.has(obj)) {
        return cache.get(obj);
    }
    var newObj = {
        __proto__: null
    };
    var hasPropertyDescriptor = Object.defineProperty && Object.getOwnPropertyDescriptor;
    for(var key in obj){
        if (key !== "default" && Object.prototype.hasOwnProperty.call(obj, key)) {
            var desc = hasPropertyDescriptor ? Object.getOwnPropertyDescriptor(obj, key) : null;
            if (desc && (desc.get || desc.set)) {
                Object.defineProperty(newObj, key, desc);
            } else {
                newObj[key] = obj[key];
            }
        }
    }
    newObj.default = obj;
    if (cache) {
        cache.set(obj, newObj);
    }
    return newObj;
}
const isEdgeRuntime = process.env.NEXT_RUNTIME === 'edge';
const debug = process.env.NEXT_PRIVATE_DEBUG_CACHE ? console.debug.bind(console, 'use-cache:') : undefined;
const filterStackFrame = process.env.NODE_ENV !== 'production' ? require('../lib/source-maps').filterStackFrameDEV : undefined;
const findSourceMapURL = process.env.NODE_ENV !== 'production' ? require('../lib/source-maps').findSourceMapURLDEV : undefined;
function generateCacheEntry(workStore, cacheContext, clientReferenceManifest, encodedArguments, fn, timeoutError) {
    // We need to run this inside a clean AsyncLocalStorage snapshot so that the cache
    // generation cannot read anything from the context we're currently executing which
    // might include request specific things like cookies() inside a React.cache().
    // Note: It is important that we await at least once before this because it lets us
    // pop out of any stack specific contexts as well - aka "Sync" Local Storage.
    return workStore.runInCleanSnapshot(generateCacheEntryWithRestoredWorkStore, workStore, cacheContext, clientReferenceManifest, encodedArguments, fn, timeoutError);
}
function generateCacheEntryWithRestoredWorkStore(workStore, cacheContext, clientReferenceManifest, encodedArguments, fn, timeoutError) {
    // Since we cleared the AsyncLocalStorage we need to restore the workStore.
    // Note: We explicitly don't restore the RequestStore nor the PrerenderStore.
    // We don't want any request specific information leaking an we don't want to create a
    // bloated fake request mock for every cache call. So any feature that currently lives
    // in RequestStore but should be available to Caches need to move to WorkStore.
    // PrerenderStore is not needed inside the cache scope because the outer most one will
    // be the one to report its result to the outer Prerender.
    return _workasyncstorageexternal.workAsyncStorage.run(workStore, generateCacheEntryWithCacheContext, workStore, cacheContext, clientReferenceManifest, encodedArguments, fn, timeoutError);
}
function createUseCacheStore(workStore, cacheContext, defaultCacheLife) {
    if (cacheContext.kind === 'private') {
        const outerWorkUnitStore = cacheContext.outerWorkUnitStore;
        return {
            type: 'private-cache',
            phase: 'render',
            implicitTags: outerWorkUnitStore == null ? void 0 : outerWorkUnitStore.implicitTags,
            revalidate: defaultCacheLife.revalidate,
            expire: defaultCacheLife.expire,
            stale: defaultCacheLife.stale,
            explicitRevalidate: undefined,
            explicitExpire: undefined,
            explicitStale: undefined,
            tags: null,
            hmrRefreshHash: (0, _workunitasyncstorageexternal.getHmrRefreshHash)(workStore, outerWorkUnitStore),
            isHmrRefresh: (0, _workunitasyncstorageexternal.isHmrRefresh)(workStore, outerWorkUnitStore),
            serverComponentsHmrCache: (0, _workunitasyncstorageexternal.getServerComponentsHmrCache)(workStore, outerWorkUnitStore),
            forceRevalidate: shouldForceRevalidate(workStore, outerWorkUnitStore),
            runtimeStagePromise: (0, _workunitasyncstorageexternal.getRuntimeStagePromise)(outerWorkUnitStore),
            draftMode: (0, _workunitasyncstorageexternal.getDraftModeProviderForCacheScope)(workStore, outerWorkUnitStore),
            rootParams: outerWorkUnitStore.rootParams,
            headers: outerWorkUnitStore.headers,
            cookies: outerWorkUnitStore.cookies
        };
    } else {
        let useCacheOrRequestStore;
        const outerWorkUnitStore = cacheContext.outerWorkUnitStore;
        if (outerWorkUnitStore) {
            switch(outerWorkUnitStore == null ? void 0 : outerWorkUnitStore.type){
                case 'cache':
                case 'private-cache':
                case 'request':
                    useCacheOrRequestStore = outerWorkUnitStore;
                    break;
                case 'prerender-runtime':
                case 'prerender':
                case 'prerender-ppr':
                case 'prerender-legacy':
                case 'unstable-cache':
                    break;
                default:
                    outerWorkUnitStore;
            }
        }
        return {
            type: 'cache',
            phase: 'render',
            implicitTags: outerWorkUnitStore == null ? void 0 : outerWorkUnitStore.implicitTags,
            revalidate: defaultCacheLife.revalidate,
            expire: defaultCacheLife.expire,
            stale: defaultCacheLife.stale,
            explicitRevalidate: undefined,
            explicitExpire: undefined,
            explicitStale: undefined,
            tags: null,
            hmrRefreshHash: outerWorkUnitStore && (0, _workunitasyncstorageexternal.getHmrRefreshHash)(workStore, outerWorkUnitStore),
            isHmrRefresh: (useCacheOrRequestStore == null ? void 0 : useCacheOrRequestStore.isHmrRefresh) ?? false,
            serverComponentsHmrCache: useCacheOrRequestStore == null ? void 0 : useCacheOrRequestStore.serverComponentsHmrCache,
            forceRevalidate: shouldForceRevalidate(workStore, outerWorkUnitStore),
            draftMode: outerWorkUnitStore && (0, _workunitasyncstorageexternal.getDraftModeProviderForCacheScope)(workStore, outerWorkUnitStore)
        };
    }
}
function assertDefaultCacheLife(defaultCacheLife) {
    if (!defaultCacheLife || defaultCacheLife.revalidate == null || defaultCacheLife.expire == null || defaultCacheLife.stale == null) {
        throw Object.defineProperty(new _invarianterror.InvariantError('A default cacheLife profile must always be provided.'), "__NEXT_ERROR_CODE", {
            value: "E750",
            enumerable: false,
            configurable: true
        });
    }
}
function generateCacheEntryWithCacheContext(workStore, cacheContext, clientReferenceManifest, encodedArguments, fn, timeoutError) {
    if (!workStore.cacheLifeProfiles) {
        throw Object.defineProperty(new _invarianterror.InvariantError('cacheLifeProfiles should always be provided.'), "__NEXT_ERROR_CODE", {
            value: "E748",
            enumerable: false,
            configurable: true
        });
    }
    const defaultCacheLife = workStore.cacheLifeProfiles['default'];
    assertDefaultCacheLife(defaultCacheLife);
    // Initialize the Store for this Cache entry.
    const cacheStore = createUseCacheStore(workStore, cacheContext, defaultCacheLife);
    return _workunitasyncstorageexternal.workUnitAsyncStorage.run(cacheStore, ()=>_dynamicaccessasyncstorageexternal.dynamicAccessAsyncStorage.run({
            abortController: new AbortController()
        }, generateCacheEntryImpl, workStore, cacheContext, cacheStore, clientReferenceManifest, encodedArguments, fn, timeoutError));
}
function propagateCacheLifeAndTagsToRevalidateStore(revalidateStore, entry) {
    const outerTags = revalidateStore.tags ??= [];
    for (const tag of entry.tags){
        if (!outerTags.includes(tag)) {
            outerTags.push(tag);
        }
    }
    if (revalidateStore.stale > entry.stale) {
        revalidateStore.stale = entry.stale;
    }
    if (revalidateStore.revalidate > entry.revalidate) {
        revalidateStore.revalidate = entry.revalidate;
    }
    if (revalidateStore.expire > entry.expire) {
        revalidateStore.expire = entry.expire;
    }
}
function propagateCacheLifeAndTags(cacheContext, entry) {
    if (cacheContext.kind === 'private') {
        switch(cacheContext.outerWorkUnitStore.type){
            case 'prerender-runtime':
            case 'private-cache':
                propagateCacheLifeAndTagsToRevalidateStore(cacheContext.outerWorkUnitStore, entry);
                break;
            case 'request':
            case undefined:
                break;
            default:
                cacheContext.outerWorkUnitStore;
        }
    } else {
        var _cacheContext_outerWorkUnitStore;
        switch((_cacheContext_outerWorkUnitStore = cacheContext.outerWorkUnitStore) == null ? void 0 : _cacheContext_outerWorkUnitStore.type){
            case 'cache':
            case 'private-cache':
            case 'prerender':
            case 'prerender-runtime':
            case 'prerender-ppr':
            case 'prerender-legacy':
                propagateCacheLifeAndTagsToRevalidateStore(cacheContext.outerWorkUnitStore, entry);
                break;
            case 'request':
            case 'unstable-cache':
            case undefined:
                break;
            default:
                cacheContext.outerWorkUnitStore;
        }
    }
}
async function collectResult(savedStream, workStore, cacheContext, innerCacheStore, startTime, errors) {
    // We create a buffered stream that collects all chunks until the end to
    // ensure that RSC has finished rendering and therefore we have collected
    // all tags. In the future the RSC API might allow for the equivalent of
    // the allReady Promise that exists on SSR streams.
    //
    // If something errored or rejected anywhere in the render, we close
    // the stream as errored. This lets a CacheHandler choose to save the
    // partial result up until that point for future hits for a while to avoid
    // unnecessary retries or not to retry. We use the end of the stream for
    // this to avoid another complicated side-channel. A receiver has to consider
    // that the stream might also error for other reasons anyway such as losing
    // connection.
    const buffer = [];
    const reader = savedStream.getReader();
    try {
        for(let entry; !(entry = await reader.read()).done;){
            buffer.push(entry.value);
        }
    } catch (error) {
        errors.push(error);
    }
    let idx = 0;
    const bufferStream = new ReadableStream({
        pull (controller) {
            if (workStore.invalidDynamicUsageError) {
                controller.error(workStore.invalidDynamicUsageError);
            } else if (idx < buffer.length) {
                controller.enqueue(buffer[idx++]);
            } else if (errors.length > 0) {
                // TODO: Should we use AggregateError here?
                controller.error(errors[0]);
            } else {
                controller.close();
            }
        }
    });
    const collectedTags = innerCacheStore.tags;
    // If cacheLife() was used to set an explicit revalidate time we use that.
    // Otherwise, we use the lowest of all inner fetch()/unstable_cache() or nested "use cache".
    // If they're lower than our default.
    const collectedRevalidate = innerCacheStore.explicitRevalidate !== undefined ? innerCacheStore.explicitRevalidate : innerCacheStore.revalidate;
    const collectedExpire = innerCacheStore.explicitExpire !== undefined ? innerCacheStore.explicitExpire : innerCacheStore.expire;
    const collectedStale = innerCacheStore.explicitStale !== undefined ? innerCacheStore.explicitStale : innerCacheStore.stale;
    const entry = {
        value: bufferStream,
        timestamp: startTime,
        revalidate: collectedRevalidate,
        expire: collectedExpire,
        stale: collectedStale,
        tags: collectedTags === null ? [] : collectedTags
    };
    if (cacheContext.outerWorkUnitStore) {
        const outerWorkUnitStore = cacheContext.outerWorkUnitStore;
        // Propagate cache life & tags to the parent context if appropriate.
        switch(outerWorkUnitStore.type){
            case 'prerender':
            case 'prerender-runtime':
                {
                    break;
                }
            case 'request':
                {
                    if (process.env.NODE_ENV === 'development' && outerWorkUnitStore.cacheSignal) {
                        break;
                    }
                // fallthrough
                }
            case 'private-cache':
            case 'cache':
            case 'unstable-cache':
            case 'prerender-legacy':
            case 'prerender-ppr':
                {
                    propagateCacheLifeAndTags(cacheContext, entry);
                    break;
                }
            default:
                {
                    outerWorkUnitStore;
                }
        }
        const cacheSignal = (0, _workunitasyncstorageexternal.getCacheSignal)(outerWorkUnitStore);
        if (cacheSignal) {
            cacheSignal.endRead();
        }
    }
    return entry;
}
async function generateCacheEntryImpl(workStore, cacheContext, innerCacheStore, clientReferenceManifest, encodedArguments, fn, timeoutError) {
    const temporaryReferences = (0, _server.createTemporaryReferenceSet)();
    const outerWorkUnitStore = cacheContext.outerWorkUnitStore;
    const [, , args] = typeof encodedArguments === 'string' ? await (0, _server.decodeReply)(encodedArguments, (0, _manifestssingleton.getServerModuleMap)(), {
        temporaryReferences
    }) : await (0, _server.decodeReplyFromAsyncIterable)({
        async *[Symbol.asyncIterator] () {
            for (const entry of encodedArguments){
                yield entry;
            }
            if (outerWorkUnitStore) {
                switch(outerWorkUnitStore.type){
                    case 'prerender-runtime':
                    case 'prerender':
                        // The encoded arguments might contain hanging promises. In
                        // this case we don't want to reject with "Error: Connection
                        // closed.", so we intentionally keep the iterable alive.
                        // This is similar to the halting trick that we do while
                        // rendering.
                        await new Promise((resolve)=>{
                            if (outerWorkUnitStore.renderSignal.aborted) {
                                resolve();
                            } else {
                                outerWorkUnitStore.renderSignal.addEventListener('abort', ()=>resolve(), {
                                    once: true
                                });
                            }
                        });
                        break;
                    case 'prerender-ppr':
                    case 'prerender-legacy':
                    case 'request':
                    case 'cache':
                    case 'private-cache':
                    case 'unstable-cache':
                        break;
                    default:
                        outerWorkUnitStore;
                }
            }
        }
    }, (0, _manifestssingleton.getServerModuleMap)(), {
        temporaryReferences
    });
    // Track the timestamp when we started computing the result.
    const startTime = performance.timeOrigin + performance.now();
    // Invoke the inner function to load a new result. We delay the invocation
    // though, until React awaits the promise so that React's request store (ALS)
    // is available when the function is invoked. This allows us, for example, to
    // capture logs so that we can later replay them.
    const resultPromise = (0, _lazyresult.createLazyResult)(fn.bind(null, ...args));
    let errors = [];
    // In the "Cache" environment, we only need to make sure that the error
    // digests are handled correctly. Error formatting and reporting is not
    // necessary here; the errors are encoded in the stream, and will be reported
    // in the "Server" environment.
    const handleError = (0, _createerrorhandler.createReactServerErrorHandler)(workStore.dev, workStore.isBuildTimePrerendering ?? false, workStore.reactServerErrorsByDigest, (error)=>{
        // In production, we log the original error here. It gets a digest that
        // can be used to associate the error with the obfuscated error that might
        // be logged if the error is caught. In development, we prefer logging the
        // transported error in the server environment. It's not obfuscated and
        // also includes the (dev-only) environment name.
        if (process.env.NODE_ENV === 'production') {
            _log.error(error);
        }
        errors.push(error);
    });
    let stream;
    switch(outerWorkUnitStore == null ? void 0 : outerWorkUnitStore.type){
        case 'prerender-runtime':
        case 'prerender':
            var _dynamicAccessAsyncStorage_getStore;
            const timeoutAbortController = new AbortController();
            // If we're prerendering, we give you 50 seconds to fill a cache entry.
            // Otherwise we assume you stalled on hanging input and de-opt. This needs
            // to be lower than just the general timeout of 60 seconds.
            const timer = setTimeout(()=>{
                workStore.invalidDynamicUsageError = timeoutError;
                timeoutAbortController.abort(timeoutError);
            }, 50000);
            const dynamicAccessAbortSignal = (_dynamicAccessAsyncStorage_getStore = _dynamicaccessasyncstorageexternal.dynamicAccessAsyncStorage.getStore()) == null ? void 0 : _dynamicAccessAsyncStorage_getStore.abortController.signal;
            const abortSignal = dynamicAccessAbortSignal ? AbortSignal.any([
                dynamicAccessAbortSignal,
                outerWorkUnitStore.renderSignal,
                timeoutAbortController.signal
            ]) : timeoutAbortController.signal;
            const { prelude } = await (0, _static.prerender)(resultPromise, clientReferenceManifest.clientModules, {
                environmentName: 'Cache',
                filterStackFrame,
                signal: abortSignal,
                temporaryReferences,
                onError (error) {
                    if (abortSignal.aborted && abortSignal.reason === error) {
                        return undefined;
                    }
                    return handleError(error);
                }
            });
            clearTimeout(timer);
            if (timeoutAbortController.signal.aborted) {
                // When the timeout is reached we always error the stream. Even for
                // fallback shell prerenders we don't want to return a hanging promise,
                // which would allow the function to become a dynamic hole. Because that
                // would mean that a non-empty shell could be generated which would be
                // subject to revalidation, and we don't want to create long
                // revalidation times.
                stream = new ReadableStream({
                    start (controller) {
                        controller.error(timeoutAbortController.signal.reason);
                    }
                });
            } else if (dynamicAccessAbortSignal == null ? void 0 : dynamicAccessAbortSignal.aborted) {
                // If the prerender is aborted because of dynamic access (e.g. reading
                // fallback params), we return a hanging promise. This essentially makes
                // the "use cache" function dynamic.
                const hangingPromise = (0, _dynamicrenderingutils.makeHangingPromise)(outerWorkUnitStore.renderSignal, workStore.route, 'dynamic "use cache"');
                if (outerWorkUnitStore.cacheSignal) {
                    outerWorkUnitStore.cacheSignal.endRead();
                }
                return {
                    type: 'prerender-dynamic',
                    hangingPromise
                };
            } else {
                stream = prelude;
            }
            break;
        case 'request':
            // If we're filling caches for a staged render, make sure that
            // it takes at least a task, so we'll always notice a cache miss between stages.
            //
            // TODO(restart-on-cache-miss): This is suboptimal.
            // Ideally we wouldn't need to restart for microtasky caches,
            // but the current logic for omitting short-lived caches only works correctly
            // if we do a second render, so that's the best we can do until we refactor that.
            if (process.env.NODE_ENV === 'development' && outerWorkUnitStore.cacheSignal) {
                await new Promise((resolve)=>setTimeout(resolve));
            }
        // fallthrough
        case 'prerender-ppr':
        case 'prerender-legacy':
        case 'cache':
        case 'private-cache':
        case 'unstable-cache':
        case undefined:
            stream = (0, _server.renderToReadableStream)(resultPromise, clientReferenceManifest.clientModules, {
                environmentName: 'Cache',
                filterStackFrame,
                temporaryReferences,
                onError: handleError
            });
            break;
        default:
            return outerWorkUnitStore;
    }
    const [returnStream, savedStream] = stream.tee();
    const pendingCacheEntry = collectResult(savedStream, workStore, cacheContext, innerCacheStore, startTime, errors);
    if (process.env.NODE_ENV === 'development') {
        // Name the stream for React DevTools.
        // @ts-expect-error
        returnStream.name = 'use cache';
    }
    return {
        type: 'cached',
        // Return the stream as we're creating it. This means that if it ends up
        // erroring we cannot return a stale-if-error version but it allows
        // streaming back the result earlier.
        stream: returnStream,
        pendingCacheEntry
    };
}
function cloneCacheEntry(entry) {
    const [streamA, streamB] = entry.value.tee();
    entry.value = streamA;
    const clonedEntry = {
        value: streamB,
        timestamp: entry.timestamp,
        revalidate: entry.revalidate,
        expire: entry.expire,
        stale: entry.stale,
        tags: entry.tags
    };
    return [
        entry,
        clonedEntry
    ];
}
async function clonePendingCacheEntry(pendingCacheEntry) {
    const entry = await pendingCacheEntry;
    return cloneCacheEntry(entry);
}
async function getNthCacheEntry(split, i) {
    return (await split)[i];
}
async function encodeFormData(formData) {
    let result = '';
    for (let [key, value] of formData){
        // We don't need this key to be serializable but from a security perspective it should not be
        // possible to generate a string that looks the same from a different structure. To ensure this
        // we need a delimeter between fields but just using a delimeter is not enough since a string
        // might contain that delimeter. We use the length of each field as the delimeter to avoid
        // escaping the values.
        result += key.length.toString(16) + ':' + key;
        let stringValue;
        if (typeof value === 'string') {
            stringValue = value;
        } else {
            // The FormData might contain binary data that is not valid UTF-8 so this cache
            // key may generate a UCS-2 string. Passing this to another service needs to be
            // aware that the key might not be compatible.
            const arrayBuffer = await value.arrayBuffer();
            if (arrayBuffer.byteLength % 2 === 0) {
                stringValue = String.fromCodePoint(...new Uint16Array(arrayBuffer));
            } else {
                stringValue = String.fromCodePoint(...new Uint16Array(arrayBuffer, 0, (arrayBuffer.byteLength - 1) / 2)) + String.fromCodePoint(new Uint8Array(arrayBuffer, arrayBuffer.byteLength - 1, 1)[0]);
            }
        }
        result += stringValue.length.toString(16) + ':' + stringValue;
    }
    return result;
}
function createTrackedReadableStream(stream, cacheSignal) {
    const reader = stream.getReader();
    return new ReadableStream({
        async pull (controller) {
            const { done, value } = await reader.read();
            if (done) {
                controller.close();
                cacheSignal.endRead();
            } else {
                controller.enqueue(value);
            }
        }
    });
}
async function cache(kind, id, boundArgsLength, originalFn, argsObj) {
    let args = Array.prototype.slice.call(argsObj);
    const isPrivate = kind === 'private';
    // Private caches are currently only stored in the Resume Data Cache (RDC),
    // and not in cache handlers.
    const cacheHandler = isPrivate ? undefined : (0, _handlers.getCacheHandler)(kind);
    if (!isPrivate && !cacheHandler) {
        throw Object.defineProperty(new Error('Unknown cache handler: ' + kind), "__NEXT_ERROR_CODE", {
            value: "E248",
            enumerable: false,
            configurable: true
        });
    }
    const timeoutError = new _usecacheerrors.UseCacheTimeoutError();
    Error.captureStackTrace(timeoutError, cache);
    const wrapAsInvalidDynamicUsageError = (error, workStore)=>{
        Error.captureStackTrace(error, cache);
        workStore.invalidDynamicUsageError ??= error;
        return error;
    };
    const workStore = _workasyncstorageexternal.workAsyncStorage.getStore();
    if (workStore === undefined) {
        throw Object.defineProperty(new Error('"use cache" cannot be used outside of App Router. Expected a WorkStore.'), "__NEXT_ERROR_CODE", {
            value: "E279",
            enumerable: false,
            configurable: true
        });
    }
    const workUnitStore = _workunitasyncstorageexternal.workUnitAsyncStorage.getStore();
    const name = originalFn.name;
    let fn = originalFn;
    let cacheContext;
    if (isPrivate) {
        const expression = '"use cache: private"';
        switch(workUnitStore == null ? void 0 : workUnitStore.type){
            // "use cache: private" is dynamic in prerendering contexts.
            case 'prerender':
                return (0, _dynamicrenderingutils.makeHangingPromise)(workUnitStore.renderSignal, workStore.route, expression);
            case 'prerender-ppr':
                return (0, _dynamicrendering.postponeWithTracking)(workStore.route, expression, workUnitStore.dynamicTracking);
            case 'prerender-legacy':
                return (0, _dynamicrendering.throwToInterruptStaticGeneration)(expression, workStore, workUnitStore);
            case 'prerender-client':
                throw Object.defineProperty(new _invarianterror.InvariantError(`${expression} must not be used within a client component. Next.js should be preventing ${expression} from being allowed in client components statically, but did not in this case.`), "__NEXT_ERROR_CODE", {
                    value: "E741",
                    enumerable: false,
                    configurable: true
                });
            case 'unstable-cache':
                {
                    throw wrapAsInvalidDynamicUsageError(Object.defineProperty(new Error(// TODO: Add a link to an error documentation page when we have one.
                    `${expression} must not be used within \`unstable_cache()\`.`), "__NEXT_ERROR_CODE", {
                        value: "E744",
                        enumerable: false,
                        configurable: true
                    }), workStore);
                }
            case 'cache':
                {
                    throw wrapAsInvalidDynamicUsageError(Object.defineProperty(new Error(// TODO: Add a link to an error documentation page when we have one.
                    `${expression} must not be used within "use cache". It can only be nested inside of another ${expression}.`), "__NEXT_ERROR_CODE", {
                        value: "E735",
                        enumerable: false,
                        configurable: true
                    }), workStore);
                }
            case 'request':
            case 'prerender-runtime':
            case 'private-cache':
                cacheContext = {
                    kind: 'private',
                    outerWorkUnitStore: workUnitStore
                };
                break;
            case undefined:
                throw wrapAsInvalidDynamicUsageError(Object.defineProperty(new Error(// TODO: Add a link to an error documentation page when we have one.
                `${expression} cannot be used outside of a request context.`), "__NEXT_ERROR_CODE", {
                    value: "E754",
                    enumerable: false,
                    configurable: true
                }), workStore);
            default:
                workUnitStore;
                // This is dead code, but without throwing an error here, TypeScript
                // will assume that cacheContext is used before being assigned.
                throw Object.defineProperty(new _invarianterror.InvariantError(`Unexpected work unit store.`), "__NEXT_ERROR_CODE", {
                    value: "E737",
                    enumerable: false,
                    configurable: true
                });
        }
    } else {
        switch(workUnitStore == null ? void 0 : workUnitStore.type){
            case 'prerender-client':
                const expression = '"use cache"';
                throw Object.defineProperty(new _invarianterror.InvariantError(`${expression} must not be used within a client component. Next.js should be preventing ${expression} from being allowed in client components statically, but did not in this case.`), "__NEXT_ERROR_CODE", {
                    value: "E741",
                    enumerable: false,
                    configurable: true
                });
            case 'prerender':
            case 'prerender-runtime':
            case 'prerender-ppr':
            case 'prerender-legacy':
            case 'request':
            case 'cache':
            case 'private-cache':
            // TODO: We should probably forbid nesting "use cache" inside
            // unstable_cache. (fallthrough)
            case 'unstable-cache':
            case undefined:
                cacheContext = {
                    kind: 'public',
                    outerWorkUnitStore: workUnitStore
                };
                break;
            default:
                workUnitStore;
                // This is dead code, but without throwing an error here, TypeScript
                // will assume that cacheContext is used before being assigned.
                throw Object.defineProperty(new _invarianterror.InvariantError(`Unexpected work unit store.`), "__NEXT_ERROR_CODE", {
                    value: "E737",
                    enumerable: false,
                    configurable: true
                });
        }
    }
    // Get the clientReferenceManifest while we're still in the outer Context.
    // In case getClientReferenceManifestSingleton is implemented using AsyncLocalStorage.
    const clientReferenceManifest = (0, _manifestssingleton.getClientReferenceManifest)();
    // Because the Action ID is not yet unique per implementation of that Action we can't
    // safely reuse the results across builds yet. In the meantime we add the buildId to the
    // arguments as a seed to ensure they're not reused. Remove this once Action IDs hash
    // the implementation.
    const buildId = workStore.buildId;
    // In dev mode, when the HMR refresh hash is set, we include it in the
    // cache key. This ensures that cache entries are not reused when server
    // components have been edited. This is a very coarse approach. But it's
    // also only a temporary solution until Action IDs are unique per
    // implementation. Remove this once Action IDs hash the implementation.
    const hmrRefreshHash = workUnitStore && (0, _workunitasyncstorageexternal.getHmrRefreshHash)(workStore, workUnitStore);
    const hangingInputAbortSignal = workUnitStore ? (0, _dynamicrendering.createHangingInputAbortSignal)(workUnitStore) : undefined;
    if (cacheContext.kind === 'private') {
        const { outerWorkUnitStore } = cacheContext;
        switch(outerWorkUnitStore.type){
            case 'prerender-runtime':
                {
                    // In a runtime prerender, we have to make sure that APIs that would hang during a static prerender
                    // are resolved with a delay, in the runtime stage. Private caches are one of these.
                    if (outerWorkUnitStore.runtimeStagePromise) {
                        await outerWorkUnitStore.runtimeStagePromise;
                    }
                    break;
                }
            case 'request':
                {
                    if (process.env.NODE_ENV === 'development') {
                        // Similar to runtime prerenders, private caches should not resolve in the static stage
                        // of a dev request, so we delay them.
                        await (0, _dynamicrenderingutils.makeDevtoolsIOAwarePromise)(undefined, outerWorkUnitStore, _stagedrendering.RenderStage.Runtime);
                    }
                    break;
                }
            case 'private-cache':
                break;
            default:
                {
                    outerWorkUnitStore;
                }
        }
    }
    let isPageOrLayoutSegmentFunction = false;
    // For page and layout segment functions (i.e. the page/layout component,
    // or generateMetadata/generateViewport), the cache function is
    // overwritten, which allows us to apply special handling for params and
    // searchParams. For pages and layouts we're using the outer params prop,
    // and not the inner one that was serialized/deserialized. While it's not
    // generally true for "use cache" args, in the case of `params` the inner
    // and outer object are essentially equivalent, so this is safe to do
    // (including fallback params that are hanging promises). It allows us to
    // avoid waiting for the timeout, when prerendering a fallback shell of a
    // cached page or layout that awaits params.
    if (isPageSegmentFunction(args)) {
        isPageOrLayoutSegmentFunction = true;
        const [{ params: outerParams, searchParams: outerSearchParams }, ...otherOuterArgs] = args;
        const props = {
            params: outerParams
        };
        if (isPrivate) {
            // Private caches allow accessing search params. We need to include
            // them in the serialized args and when generating the cache key.
            props.searchParams = outerSearchParams;
        }
        args = [
            props,
            ...otherOuterArgs
        ];
        fn = ({
            [name]: async ({ params: _innerParams, searchParams: innerSearchParams }, ...otherInnerArgs)=>originalFn.apply(null, [
                    {
                        params: outerParams,
                        searchParams: innerSearchParams ?? // For public caches, search params are omitted from the cache
                        // key (and the serialized args) to avoid mismatches between
                        // prerendering and resuming a cached page that does not
                        // access search params. This is also the reason why we're not
                        // using a hanging promise for search params. For cached pages
                        // that do access them, which is an invalid dynamic usage, we
                        // need to ensure that an error is shown.
                        (0, _searchparams.makeErroringSearchParamsForUseCache)(workStore)
                    },
                    ...otherInnerArgs
                ])
        })[name];
    } else if (isLayoutSegmentFunction(args)) {
        isPageOrLayoutSegmentFunction = true;
        const [{ params: outerParams, $$isLayout, ...outerSlots }, ...otherOuterArgs] = args;
        // Overwrite the props to omit $$isLayout. Note that slots are only
        // passed to the layout component (if any are defined), and not to
        // generateMetadata nor generateViewport. For those functions,
        // outerSlots/innerSlots is an empty object, which is fine because we're
        // just spreading it into the props.
        args = [
            {
                params: outerParams,
                ...outerSlots
            },
            ...otherOuterArgs
        ];
        fn = ({
            [name]: async ({ params: _innerParams, ...innerSlots }, ...otherInnerArgs)=>originalFn.apply(null, [
                    {
                        params: outerParams,
                        ...innerSlots
                    },
                    ...otherInnerArgs
                ])
        })[name];
    }
    if (boundArgsLength > 0) {
        if (args.length === 0) {
            throw Object.defineProperty(new _invarianterror.InvariantError(`Expected the "use cache" function ${JSON.stringify(fn.name)} to receive its encrypted bound arguments as the first argument.`), "__NEXT_ERROR_CODE", {
                value: "E524",
                enumerable: false,
                configurable: true
            });
        }
        const encryptedBoundArgs = args.shift();
        const boundArgs = await (0, _encryption.decryptActionBoundArgs)(id, encryptedBoundArgs);
        if (!Array.isArray(boundArgs)) {
            throw Object.defineProperty(new _invarianterror.InvariantError(`Expected the bound arguments of "use cache" function ${JSON.stringify(fn.name)} to deserialize into an array, got ${typeof boundArgs} instead.`), "__NEXT_ERROR_CODE", {
                value: "E581",
                enumerable: false,
                configurable: true
            });
        }
        if (boundArgsLength !== boundArgs.length) {
            throw Object.defineProperty(new _invarianterror.InvariantError(`Expected the "use cache" function ${JSON.stringify(fn.name)} to receive ${boundArgsLength} bound arguments, got ${boundArgs.length} instead.`), "__NEXT_ERROR_CODE", {
                value: "E559",
                enumerable: false,
                configurable: true
            });
        }
        args.unshift(boundArgs);
    }
    const temporaryReferences = (0, _client.createTemporaryReferenceSet)();
    // For private caches, which are allowed to read cookies, we still don't
    // need to include the cookies in the cache key. This is because we don't
    // store the cache entries in a cache handler, but only in the Resume Data
    // Cache (RDC). Private caches are only used during dynamic requests and
    // runtime prefetches. For dynamic requests, the RDC is immutable, so it
    // does not include any private caches. For runtime prefetches, the RDC is
    // mutable, but only lives as long as the request, so the key does not
    // need to include cookies.
    const cacheKeyParts = hmrRefreshHash ? [
        buildId,
        id,
        args,
        hmrRefreshHash
    ] : [
        buildId,
        id,
        args
    ];
    const encodeCacheKeyParts = ()=>(0, _client.encodeReply)(cacheKeyParts, {
            temporaryReferences,
            signal: hangingInputAbortSignal
        });
    let encodedCacheKeyParts;
    switch(workUnitStore == null ? void 0 : workUnitStore.type){
        case 'prerender-runtime':
        // We're currently only using `dynamicAccessAsyncStorage` for params,
        // which are always available in a runtime prerender, so they will never hang,
        // effectively making the tracking below a no-op.
        // However, a runtime prerender shares a lot of the semantics with a static prerender,
        // and might need to follow this codepath in the future
        // if we start using `dynamicAccessAsyncStorage` for other APIs.
        //
        // fallthrough
        case 'prerender':
            if (!isPageOrLayoutSegmentFunction) {
                // If the "use cache" function is not a page or layout segment
                // function, we need to track dynamic access already when encoding
                // the arguments. If params are passed explicitly into a "use cache"
                // function (as opposed to receiving them automatically in a page or
                // layout), we assume that the params are also accessed. This allows
                // us to abort early, and treat the function as dynamic, instead of
                // waiting for the timeout to be reached.
                const dynamicAccessAbortController = new AbortController();
                encodedCacheKeyParts = await _dynamicaccessasyncstorageexternal.dynamicAccessAsyncStorage.run({
                    abortController: dynamicAccessAbortController
                }, encodeCacheKeyParts);
                if (dynamicAccessAbortController.signal.aborted) {
                    return (0, _dynamicrenderingutils.makeHangingPromise)(workUnitStore.renderSignal, workStore.route, 'dynamic "use cache"');
                }
                break;
            }
        // fallthrough
        case 'prerender-ppr':
        case 'prerender-legacy':
        case 'request':
        // TODO(restart-on-cache-miss): We need to handle params/searchParams on page components.
        // the promises will be tasky, so `encodeCacheKeyParts` will not resolve in the static stage.
        // We have not started a cache read at this point, so we might just miss the cache completely.
        // fallthrough
        case 'cache':
        case 'private-cache':
        case 'unstable-cache':
        case undefined:
            encodedCacheKeyParts = await encodeCacheKeyParts();
            break;
        default:
            return workUnitStore;
    }
    const serializedCacheKey = typeof encodedCacheKeyParts === 'string' ? // Convert it to an ArrayBuffer if it wants to.
    encodedCacheKeyParts : await encodeFormData(encodedCacheKeyParts);
    let stream = undefined;
    // Get an immutable and mutable versions of the resume data cache.
    const prerenderResumeDataCache = workUnitStore ? (0, _workunitasyncstorageexternal.getPrerenderResumeDataCache)(workUnitStore) : null;
    const renderResumeDataCache = workUnitStore ? (0, _workunitasyncstorageexternal.getRenderResumeDataCache)(workUnitStore) : null;
    if (renderResumeDataCache) {
        const cacheSignal = workUnitStore ? (0, _workunitasyncstorageexternal.getCacheSignal)(workUnitStore) : null;
        if (cacheSignal) {
            cacheSignal.beginRead();
        }
        const cachedEntry = renderResumeDataCache.cache.get(serializedCacheKey);
        if (cachedEntry !== undefined) {
            const existingEntry = await cachedEntry;
            if (workUnitStore !== undefined && existingEntry !== undefined) {
                if (existingEntry.revalidate === 0 || existingEntry.expire < _constants.DYNAMIC_EXPIRE) {
                    switch(workUnitStore.type){
                        case 'prerender':
                            // In a Dynamic I/O prerender, if the cache entry has
                            // revalidate: 0 or if the expire time is under 5 minutes,
                            // then we consider this cache entry dynamic as it's not worth
                            // generating static pages for such data. It's better to leave
                            // a dynamic hole that can be filled in during the resume with
                            // a potentially cached entry.
                            if (cacheSignal) {
                                cacheSignal.endRead();
                            }
                            return (0, _dynamicrenderingutils.makeHangingPromise)(workUnitStore.renderSignal, workStore.route, 'dynamic "use cache"');
                        case 'prerender-runtime':
                            {
                                // In the final phase of a runtime prerender, we have to make
                                // sure that APIs that would hang during a static prerender
                                // are resolved with a delay, in the runtime stage.
                                if (workUnitStore.runtimeStagePromise) {
                                    await workUnitStore.runtimeStagePromise;
                                }
                                break;
                            }
                        case 'request':
                            {
                                if (process.env.NODE_ENV === 'development') {
                                    // We delay the cache here so that it doesn't resolve in the static task --
                                    // in a regular static prerender, it'd be a hanging promise, and we need to reflect that,
                                    // so it has to resolve later.
                                    // TODO(restart-on-cache-miss): Optimize this to avoid unnecessary restarts.
                                    // We don't end the cache read here, so this will always appear as a cache miss in the static stage,
                                    // and thus will cause a restart even if all caches are filled.
                                    await (0, _dynamicrenderingutils.makeDevtoolsIOAwarePromise)(undefined, workUnitStore, _stagedrendering.RenderStage.Runtime);
                                }
                                break;
                            }
                        case 'prerender-ppr':
                        case 'prerender-legacy':
                        case 'cache':
                        case 'private-cache':
                        case 'unstable-cache':
                            break;
                        default:
                            workUnitStore;
                    }
                }
                if (existingEntry.stale < _constants.RUNTIME_PREFETCH_DYNAMIC_STALE) {
                    switch(workUnitStore.type){
                        case 'prerender-runtime':
                            // In a runtime prerender, if the cache entry will become
                            // stale in less then 30 seconds, we consider this cache entry
                            // dynamic as it's not worth prefetching. It's better to leave
                            // a dynamic hole that can be filled during the navigation.
                            if (cacheSignal) {
                                cacheSignal.endRead();
                            }
                            return (0, _dynamicrenderingutils.makeHangingPromise)(workUnitStore.renderSignal, workStore.route, 'dynamic "use cache"');
                        case 'request':
                            {
                                if (process.env.NODE_ENV === 'development') {
                                    // We delay the cache here so that it doesn't resolve in the runtime phase --
                                    // in a regular runtime prerender, it'd be a hanging promise, and we need to reflect that,
                                    // so it has to resolve later.
                                    // TODO(restart-on-cache-miss): Optimize this to avoid unnecessary restarts.
                                    // We don't end the cache read here, so this will always appear as a cache miss in the runtime stage,
                                    // and thus will cause a restart even if all caches are filled.
                                    await (0, _dynamicrenderingutils.makeDevtoolsIOAwarePromise)(undefined, workUnitStore, _stagedrendering.RenderStage.Dynamic);
                                }
                                break;
                            }
                        case 'prerender':
                        case 'prerender-ppr':
                        case 'prerender-legacy':
                        case 'cache':
                        case 'private-cache':
                        case 'unstable-cache':
                            break;
                        default:
                            workUnitStore;
                    }
                }
            }
            // We want to make sure we only propagate cache life & tags if the
            // entry was *not* omitted from the prerender. So we only do this
            // after the above early returns.
            propagateCacheLifeAndTags(cacheContext, existingEntry);
            const [streamA, streamB] = existingEntry.value.tee();
            existingEntry.value = streamB;
            if (cacheSignal) {
                // When we have a cacheSignal we need to block on reading the cache
                // entry before ending the read.
                stream = createTrackedReadableStream(streamA, cacheSignal);
            } else {
                stream = streamA;
            }
        } else {
            if (cacheSignal) {
                cacheSignal.endRead();
            }
            if (workUnitStore) {
                switch(workUnitStore.type){
                    case 'prerender':
                        // If `allowEmptyStaticShell` is true, and thus a prefilled
                        // resume data cache was provided, then a cache miss means that
                        // params were part of the cache key. In this case, we can make
                        // this cache function a dynamic hole in the shell (or produce
                        // an empty shell if there's no parent suspense boundary).
                        // Currently, this also includes layouts and pages that don't
                        // read params, which will be improved when we implement
                        // NAR-136. Otherwise, we assume that if params are passed
                        // explicitly into a "use cache" function, that the params are
                        // also accessed. This allows us to abort early, and treat the
                        // function as dynamic, instead of waiting for the timeout to be
                        // reached. Compared to the instrumentation-based params bailout
                        // we do here, this also covers the case where params are
                        // transformed with an async function, before being passed into
                        // the "use cache" function, which escapes the instrumentation.
                        if (workUnitStore.allowEmptyStaticShell) {
                            return (0, _dynamicrenderingutils.makeHangingPromise)(workUnitStore.renderSignal, workStore.route, 'dynamic "use cache"');
                        }
                        break;
                    case 'prerender-runtime':
                    case 'prerender-ppr':
                    case 'prerender-legacy':
                    case 'request':
                    case 'cache':
                    case 'private-cache':
                    case 'unstable-cache':
                        break;
                    default:
                        workUnitStore;
                }
            }
        }
    }
    if (stream === undefined) {
        const cacheSignal = workUnitStore ? (0, _workunitasyncstorageexternal.getCacheSignal)(workUnitStore) : null;
        if (cacheSignal) {
            // Either the cache handler or the generation can be using I/O at this point.
            // We need to track when they start and when they complete.
            cacheSignal.beginRead();
        }
        const lazyRefreshTags = workStore.refreshTagsByCacheKind.get(kind);
        if (lazyRefreshTags && !(0, _lazyresult.isResolvedLazyResult)(lazyRefreshTags)) {
            await lazyRefreshTags;
        }
        let entry;
        // We ignore existing cache entries when force revalidating.
        if (cacheHandler && !shouldForceRevalidate(workStore, workUnitStore)) {
            var _workUnitStore_implicitTags;
            entry = await cacheHandler.get(serializedCacheKey, (workUnitStore == null ? void 0 : (_workUnitStore_implicitTags = workUnitStore.implicitTags) == null ? void 0 : _workUnitStore_implicitTags.tags) ?? []);
        }
        if (entry) {
            var _workUnitStore_implicitTags1;
            const implicitTags = (workUnitStore == null ? void 0 : (_workUnitStore_implicitTags1 = workUnitStore.implicitTags) == null ? void 0 : _workUnitStore_implicitTags1.tags) ?? [];
            let implicitTagsExpiration = 0;
            if (workUnitStore == null ? void 0 : workUnitStore.implicitTags) {
                const lazyExpiration = workUnitStore.implicitTags.expirationsByCacheKind.get(kind);
                if (lazyExpiration) {
                    const expiration = (0, _lazyresult.isResolvedLazyResult)(lazyExpiration) ? lazyExpiration.value : await lazyExpiration;
                    // If a cache handler returns an expiration time of Infinity, it
                    // signals to Next.js that it handles checking cache entries for
                    // staleness based on the expiration of the implicit tags passed
                    // into the `get` method. In this case, we keep the default of 0,
                    // which means that the implicit tags are not considered expired.
                    if (expiration < Infinity) {
                        implicitTagsExpiration = expiration;
                    }
                }
            }
            if (shouldDiscardCacheEntry(entry, workStore, workUnitStore, implicitTags, implicitTagsExpiration)) {
                debug == null ? void 0 : debug('discarding expired entry', serializedCacheKey);
                entry = undefined;
            }
        }
        const currentTime = performance.timeOrigin + performance.now();
        if (workUnitStore !== undefined && entry !== undefined && (entry.revalidate === 0 || entry.expire < _constants.DYNAMIC_EXPIRE)) {
            switch(workUnitStore.type){
                case 'prerender':
                    // In a Dynamic I/O prerender, if the cache entry has revalidate:
                    // 0 or if the expire time is under 5 minutes, then we consider
                    // this cache entry dynamic as it's not worth generating static
                    // pages for such data. It's better to leave a dynamic hole that
                    // can be filled in during the resume with a potentially cached
                    // entry.
                    if (cacheSignal) {
                        cacheSignal.endRead();
                    }
                    return (0, _dynamicrenderingutils.makeHangingPromise)(workUnitStore.renderSignal, workStore.route, 'dynamic "use cache"');
                case 'request':
                    {
                        if (process.env.NODE_ENV === 'development') {
                            // We delay the cache here so that it doesn't resolve in the static task --
                            // in a regular static prerender, it'd be a hanging promise, and we need to reflect that,
                            // so it has to resolve later.
                            // TODO(restart-on-cache-miss): Optimize this to avoid unnecessary restarts.
                            // We don't end the cache read here, so this will always appear as a cache miss in the static stage,
                            // and thus will cause a restart even if all caches are filled.
                            await (0, _dynamicrenderingutils.makeDevtoolsIOAwarePromise)(undefined, workUnitStore, _stagedrendering.RenderStage.Runtime);
                        }
                        break;
                    }
                case 'prerender-runtime':
                case 'prerender-ppr':
                case 'prerender-legacy':
                case 'cache':
                case 'private-cache':
                case 'unstable-cache':
                    break;
                default:
                    workUnitStore;
            }
        }
        if (entry === undefined || currentTime > entry.timestamp + entry.expire * 1000 || workStore.isStaticGeneration && currentTime > entry.timestamp + entry.revalidate * 1000) {
            // Miss. Generate a new result.
            // If the cache entry is stale and we're prerendering, we don't want to use the
            // stale entry since it would unnecessarily need to shorten the lifetime of the
            // prerender. We're not time constrained here so we can re-generated it now.
            // We need to run this inside a clean AsyncLocalStorage snapshot so that the cache
            // generation cannot read anything from the context we're currently executing which
            // might include request specific things like cookies() inside a React.cache().
            // Note: It is important that we await at least once before this because it lets us
            // pop out of any stack specific contexts as well - aka "Sync" Local Storage.
            if (entry) {
                if (currentTime > entry.timestamp + entry.expire * 1000) {
                    debug == null ? void 0 : debug('entry is expired', serializedCacheKey);
                }
                if (workStore.isStaticGeneration && currentTime > entry.timestamp + entry.revalidate * 1000) {
                    debug == null ? void 0 : debug('static generation, entry is stale', serializedCacheKey);
                }
            }
            const result = await generateCacheEntry(workStore, cacheContext, clientReferenceManifest, encodedCacheKeyParts, fn, timeoutError);
            if (result.type === 'prerender-dynamic') {
                return result.hangingPromise;
            }
            const { stream: newStream, pendingCacheEntry } = result;
            // When draft mode is enabled, we must not save the cache entry.
            if (!workStore.isDraftMode) {
                let savedCacheEntry;
                if (prerenderResumeDataCache) {
                    // Create a clone that goes into the cache scope memory cache.
                    const split = clonePendingCacheEntry(pendingCacheEntry);
                    savedCacheEntry = getNthCacheEntry(split, 0);
                    prerenderResumeDataCache.cache.set(serializedCacheKey, getNthCacheEntry(split, 1));
                } else {
                    savedCacheEntry = pendingCacheEntry;
                }
                if (cacheHandler) {
                    const promise = cacheHandler.set(serializedCacheKey, savedCacheEntry);
                    workStore.pendingRevalidateWrites ??= [];
                    workStore.pendingRevalidateWrites.push(promise);
                }
            }
            stream = newStream;
        } else {
            // If we have an entry at this point, this can't be a private cache
            // entry.
            if (cacheContext.kind === 'private') {
                throw Object.defineProperty(new _invarianterror.InvariantError(`A private cache entry must not be retrieved from the cache handler.`), "__NEXT_ERROR_CODE", {
                    value: "E749",
                    enumerable: false,
                    configurable: true
                });
            }
            propagateCacheLifeAndTags(cacheContext, entry);
            // We want to return this stream, even if it's stale.
            stream = entry.value;
            // If we have a cache scope, we need to clone the entry and set it on
            // the inner cache scope.
            if (prerenderResumeDataCache) {
                const [entryLeft, entryRight] = cloneCacheEntry(entry);
                if (cacheSignal) {
                    stream = createTrackedReadableStream(entryLeft.value, cacheSignal);
                } else {
                    stream = entryLeft.value;
                }
                prerenderResumeDataCache.cache.set(serializedCacheKey, Promise.resolve(entryRight));
            } else {
                // If we're not regenerating we need to signal that we've finished
                // putting the entry into the cache scope at this point. Otherwise we do
                // that inside generateCacheEntry.
                cacheSignal == null ? void 0 : cacheSignal.endRead();
            }
            if (currentTime > entry.timestamp + entry.revalidate * 1000) {
                // If this is stale, and we're not in a prerender (i.e. this is
                // dynamic render), then we should warm up the cache with a fresh
                // revalidated entry.
                const result = await generateCacheEntry(workStore, // This is not running within the context of this unit.
                {
                    kind: cacheContext.kind,
                    outerWorkUnitStore: undefined
                }, clientReferenceManifest, encodedCacheKeyParts, fn, timeoutError);
                if (result.type === 'cached') {
                    const { stream: ignoredStream, pendingCacheEntry } = result;
                    let savedCacheEntry;
                    if (prerenderResumeDataCache) {
                        const split = clonePendingCacheEntry(pendingCacheEntry);
                        savedCacheEntry = getNthCacheEntry(split, 0);
                        prerenderResumeDataCache.cache.set(serializedCacheKey, getNthCacheEntry(split, 1));
                    } else {
                        savedCacheEntry = pendingCacheEntry;
                    }
                    if (cacheHandler) {
                        const promise = cacheHandler.set(serializedCacheKey, savedCacheEntry);
                        workStore.pendingRevalidateWrites ??= [];
                        workStore.pendingRevalidateWrites.push(promise);
                    }
                    await ignoredStream.cancel();
                }
            }
        }
    }
    // Logs are replayed even if it's a hit - to ensure we see them on the client eventually.
    // If we didn't then the client wouldn't see the logs if it was seeded from a prewarm that
    // never made it to the client. However, this also means that you see logs even when the
    // cached function isn't actually re-executed. We should instead ensure prewarms always
    // make it to the client. Another issue is that this will cause double logging in the
    // server terminal. Once while generating the cache entry and once when replaying it on
    // the server, which is required to pick it up for replaying again on the client.
    const replayConsoleLogs = true;
    const serverConsumerManifest = {
        // moduleLoading must be null because we don't want to trigger preloads of ClientReferences
        // to be added to the consumer. Instead, we'll wait for any ClientReference to be emitted
        // which themselves will handle the preloading.
        moduleLoading: null,
        moduleMap: isEdgeRuntime ? clientReferenceManifest.edgeRscModuleMapping : clientReferenceManifest.rscModuleMapping,
        serverModuleMap: (0, _manifestssingleton.getServerModuleMap)()
    };
    return (0, _client.createFromReadableStream)(stream, {
        findSourceMapURL,
        serverConsumerManifest,
        temporaryReferences,
        replayConsoleLogs,
        environmentName: 'Cache'
    });
}
/**
 * Returns `true` if the `'use cache'` function is the page component itself,
 * or `generateMetadata`/`generateViewport` in a page file.
 */ function isPageSegmentFunction(args) {
    const [maybeProps] = args;
    return maybeProps !== null && typeof maybeProps === 'object' && maybeProps.$$isPage === true;
}
/**
 * Returns `true` if the `'use cache'` function is the layout component itself,
 * or `generateMetadata`/`generateViewport` in a layout file.
 */ function isLayoutSegmentFunction(args) {
    const [maybeProps] = args;
    return maybeProps !== null && typeof maybeProps === 'object' && maybeProps.$$isLayout === true;
}
function shouldForceRevalidate(workStore, workUnitStore) {
    if (workStore.isOnDemandRevalidate || workStore.isDraftMode) {
        return true;
    }
    if (workStore.dev && workUnitStore) {
        switch(workUnitStore.type){
            case 'request':
                return workUnitStore.headers.get('cache-control') === 'no-cache';
            case 'cache':
            case 'private-cache':
                return workUnitStore.forceRevalidate;
            case 'prerender-runtime':
            case 'prerender':
            case 'prerender-client':
            case 'prerender-ppr':
            case 'prerender-legacy':
            case 'unstable-cache':
                break;
            default:
                workUnitStore;
        }
    }
    return false;
}
function shouldDiscardCacheEntry(entry, workStore, workUnitStore, implicitTags, implicitTagsExpiration) {
    // If the cache entry was created before any of the implicit tags were
    // revalidated last, we need to discard it.
    if (entry.timestamp <= implicitTagsExpiration) {
        debug == null ? void 0 : debug('entry was created at', entry.timestamp, 'before implicit tags were revalidated at', implicitTagsExpiration);
        return true;
    }
    // During prerendering, we ignore recently revalidated tags. In dev mode, we
    // can assume that the dynamic dev rendering will have discarded and recreated
    // the affected cache entries, and we don't want to discard those again during
    // the prerender validation. During build-time prerendering, there will never
    // be any pending revalidated tags.
    if (workUnitStore) {
        switch(workUnitStore.type){
            case 'prerender':
                return false;
            case 'prerender-runtime':
            case 'prerender-client':
            case 'prerender-ppr':
            case 'prerender-legacy':
            case 'request':
            case 'cache':
            case 'private-cache':
            case 'unstable-cache':
                break;
            default:
                workUnitStore;
        }
    }
    // If the cache entry contains revalidated tags that the cache handler might
    // not know about yet, we need to discard it.
    if (entry.tags.some((tag)=>isRecentlyRevalidatedTag(tag, workStore))) {
        return true;
    }
    // Finally, if any of the implicit tags have been revalidated recently, we
    // also need to discard the cache entry.
    if (implicitTags.some((tag)=>isRecentlyRevalidatedTag(tag, workStore))) {
        return true;
    }
    return false;
}
function isRecentlyRevalidatedTag(tag, workStore) {
    const { previouslyRevalidatedTags, pendingRevalidatedTags } = workStore;
    // Was the tag previously revalidated (e.g. by a redirecting server action)?
    if (previouslyRevalidatedTags.includes(tag)) {
        debug == null ? void 0 : debug('tag', tag, 'was previously revalidated');
        return true;
    }
    // It could also have been revalidated by the currently running server action.
    // In this case the revalidation might not have been fully propagated by a
    // remote cache handler yet, so we read it from the pending tags in the work
    // store.
    if (pendingRevalidatedTags == null ? void 0 : pendingRevalidatedTags.some((item)=>item.tag === tag)) {
        debug == null ? void 0 : debug('tag', tag, 'was just revalidated');
        return true;
    }
    return false;
}

//# sourceMappingURL=use-cache-wrapper.js.map