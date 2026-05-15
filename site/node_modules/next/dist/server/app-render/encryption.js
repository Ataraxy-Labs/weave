/* eslint-disable import/no-extraneous-dependencies */ "use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
0 && (module.exports = {
    decryptActionBoundArgs: null,
    encryptActionBoundArgs: null
});
function _export(target, all) {
    for(var name in all)Object.defineProperty(target, name, {
        enumerable: true,
        get: all[name]
    });
}
_export(exports, {
    decryptActionBoundArgs: function() {
        return decryptActionBoundArgs;
    },
    encryptActionBoundArgs: function() {
        return encryptActionBoundArgs;
    }
});
require("server-only");
const _server = require("react-server-dom-webpack/server");
const _client = require("react-server-dom-webpack/client");
const _nodewebstreamshelper = require("../stream-utils/node-web-streams-helper");
const _encryptionutils = require("./encryption-utils");
const _manifestssingleton = require("./manifests-singleton");
const _workunitasyncstorageexternal = require("./work-unit-async-storage.external");
const _dynamicrendering = require("./dynamic-rendering");
const _react = /*#__PURE__*/ _interop_require_default(require("react"));
function _interop_require_default(obj) {
    return obj && obj.__esModule ? obj : {
        default: obj
    };
}
const isEdgeRuntime = process.env.NEXT_RUNTIME === 'edge';
const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder();
const filterStackFrame = process.env.NODE_ENV !== 'production' ? require('../lib/source-maps').filterStackFrameDEV : undefined;
const findSourceMapURL = process.env.NODE_ENV !== 'production' ? require('../lib/source-maps').findSourceMapURLDEV : undefined;
/**
 * Decrypt the serialized string with the action id as the salt.
 */ async function decodeActionBoundArg(actionId, arg) {
    const key = await (0, _encryptionutils.getActionEncryptionKey)();
    if (typeof key === 'undefined') {
        throw Object.defineProperty(new Error(`Missing encryption key for Server Action. This is a bug in Next.js`), "__NEXT_ERROR_CODE", {
            value: "E65",
            enumerable: false,
            configurable: true
        });
    }
    // Get the iv (16 bytes) and the payload from the arg.
    const originalPayload = atob(arg);
    const ivValue = originalPayload.slice(0, 16);
    const payload = originalPayload.slice(16);
    const decrypted = textDecoder.decode(await (0, _encryptionutils.decrypt)(key, (0, _encryptionutils.stringToUint8Array)(ivValue), (0, _encryptionutils.stringToUint8Array)(payload)));
    if (!decrypted.startsWith(actionId)) {
        throw Object.defineProperty(new Error('Invalid Server Action payload: failed to decrypt.'), "__NEXT_ERROR_CODE", {
            value: "E191",
            enumerable: false,
            configurable: true
        });
    }
    return decrypted.slice(actionId.length);
}
/**
 * Encrypt the serialized string with the action id as the salt. Add a prefix to
 * later ensure that the payload is correctly decrypted, similar to a checksum.
 */ async function encodeActionBoundArg(actionId, arg) {
    const key = await (0, _encryptionutils.getActionEncryptionKey)();
    if (key === undefined) {
        throw Object.defineProperty(new Error(`Missing encryption key for Server Action. This is a bug in Next.js`), "__NEXT_ERROR_CODE", {
            value: "E65",
            enumerable: false,
            configurable: true
        });
    }
    // Get 16 random bytes as iv.
    const randomBytes = new Uint8Array(16);
    _workunitasyncstorageexternal.workUnitAsyncStorage.exit(()=>crypto.getRandomValues(randomBytes));
    const ivValue = (0, _encryptionutils.arrayBufferToString)(randomBytes.buffer);
    const encrypted = await (0, _encryptionutils.encrypt)(key, randomBytes, textEncoder.encode(actionId + arg));
    return btoa(ivValue + (0, _encryptionutils.arrayBufferToString)(encrypted));
}
var ReadStatus = /*#__PURE__*/ function(ReadStatus) {
    ReadStatus[ReadStatus["Ready"] = 0] = "Ready";
    ReadStatus[ReadStatus["Pending"] = 1] = "Pending";
    ReadStatus[ReadStatus["Complete"] = 2] = "Complete";
    return ReadStatus;
}(ReadStatus || {});
const encryptActionBoundArgs = _react.default.cache(async function encryptActionBoundArgs(actionId, ...args) {
    const workUnitStore = _workunitasyncstorageexternal.workUnitAsyncStorage.getStore();
    const cacheSignal = workUnitStore ? (0, _workunitasyncstorageexternal.getCacheSignal)(workUnitStore) : undefined;
    const { clientModules } = (0, _manifestssingleton.getClientReferenceManifest)();
    // Create an error before any asynchronous calls, to capture the original
    // call stack in case we need it when the serialization errors.
    const error = new Error();
    Error.captureStackTrace(error, encryptActionBoundArgs);
    let didCatchError = false;
    const hangingInputAbortSignal = workUnitStore ? (0, _dynamicrendering.createHangingInputAbortSignal)(workUnitStore) : undefined;
    let readStatus = 0;
    function startReadOnce() {
        if (readStatus === 0) {
            readStatus = 1;
            cacheSignal == null ? void 0 : cacheSignal.beginRead();
        }
    }
    function endReadIfStarted() {
        if (readStatus === 1) {
            cacheSignal == null ? void 0 : cacheSignal.endRead();
        }
        readStatus = 2;
    }
    // streamToString might take longer than a microtask to resolve and then other things
    // waiting on the cache signal might not realize there is another cache to fill so if
    // we are no longer waiting on the bound args serialization via the hangingInputAbortSignal
    // we should eagerly start the cache read to prevent other readers of the cache signal from
    // missing this cache fill. We use a idempotent function to only start reading once because
    // it's also possible that streamToString finishes before the hangingInputAbortSignal aborts.
    if (hangingInputAbortSignal && cacheSignal) {
        hangingInputAbortSignal.addEventListener('abort', startReadOnce, {
            once: true
        });
    }
    // Using Flight to serialize the args into a string.
    const serialized = await (0, _nodewebstreamshelper.streamToString)((0, _server.renderToReadableStream)(args, clientModules, {
        filterStackFrame,
        signal: hangingInputAbortSignal,
        onError (err) {
            if (hangingInputAbortSignal == null ? void 0 : hangingInputAbortSignal.aborted) {
                return;
            }
            // We're only reporting one error at a time, starting with the first.
            if (didCatchError) {
                return;
            }
            didCatchError = true;
            // Use the original error message together with the previously created
            // stack, because err.stack is a useless Flight Server call stack.
            error.message = err instanceof Error ? err.message : String(err);
        }
    }), // We pass the abort signal to `streamToString` so that no chunks are
    // included that are emitted after the signal was already aborted. This
    // ensures that we can encode hanging promises.
    hangingInputAbortSignal);
    if (didCatchError) {
        if (process.env.NODE_ENV === 'development') {
            // Logging the error is needed for server functions that are passed to the
            // client where the decryption is not done during rendering. Console
            // replaying allows us to still show the error dev overlay in this case.
            console.error(error);
        }
        endReadIfStarted();
        throw error;
    }
    if (!workUnitStore) {
        // We don't need to call cacheSignal.endRead here because we can't have a cacheSignal
        // if we do not have a workUnitStore.
        return encodeActionBoundArg(actionId, serialized);
    }
    startReadOnce();
    const prerenderResumeDataCache = (0, _workunitasyncstorageexternal.getPrerenderResumeDataCache)(workUnitStore);
    const renderResumeDataCache = (0, _workunitasyncstorageexternal.getRenderResumeDataCache)(workUnitStore);
    const cacheKey = actionId + serialized;
    const cachedEncrypted = (prerenderResumeDataCache == null ? void 0 : prerenderResumeDataCache.encryptedBoundArgs.get(cacheKey)) ?? (renderResumeDataCache == null ? void 0 : renderResumeDataCache.encryptedBoundArgs.get(cacheKey));
    if (cachedEncrypted) {
        return cachedEncrypted;
    }
    const encrypted = await encodeActionBoundArg(actionId, serialized);
    endReadIfStarted();
    prerenderResumeDataCache == null ? void 0 : prerenderResumeDataCache.encryptedBoundArgs.set(cacheKey, encrypted);
    return encrypted;
});
async function decryptActionBoundArgs(actionId, encryptedPromise) {
    const encrypted = await encryptedPromise;
    const workUnitStore = _workunitasyncstorageexternal.workUnitAsyncStorage.getStore();
    let decrypted;
    if (workUnitStore) {
        const cacheSignal = (0, _workunitasyncstorageexternal.getCacheSignal)(workUnitStore);
        const prerenderResumeDataCache = (0, _workunitasyncstorageexternal.getPrerenderResumeDataCache)(workUnitStore);
        const renderResumeDataCache = (0, _workunitasyncstorageexternal.getRenderResumeDataCache)(workUnitStore);
        decrypted = (prerenderResumeDataCache == null ? void 0 : prerenderResumeDataCache.decryptedBoundArgs.get(encrypted)) ?? (renderResumeDataCache == null ? void 0 : renderResumeDataCache.decryptedBoundArgs.get(encrypted));
        if (!decrypted) {
            cacheSignal == null ? void 0 : cacheSignal.beginRead();
            decrypted = await decodeActionBoundArg(actionId, encrypted);
            cacheSignal == null ? void 0 : cacheSignal.endRead();
            prerenderResumeDataCache == null ? void 0 : prerenderResumeDataCache.decryptedBoundArgs.set(encrypted, decrypted);
        }
    } else {
        decrypted = await decodeActionBoundArg(actionId, encrypted);
    }
    const { edgeRscModuleMapping, rscModuleMapping } = (0, _manifestssingleton.getClientReferenceManifest)();
    // Using Flight to deserialize the args from the string.
    const deserialized = await (0, _client.createFromReadableStream)(new ReadableStream({
        start (controller) {
            controller.enqueue(textEncoder.encode(decrypted));
            switch(workUnitStore == null ? void 0 : workUnitStore.type){
                case 'prerender':
                case 'prerender-runtime':
                    // Explicitly don't close the stream here (until prerendering is
                    // complete) so that hanging promises are not rejected.
                    if (workUnitStore.renderSignal.aborted) {
                        controller.close();
                    } else {
                        workUnitStore.renderSignal.addEventListener('abort', ()=>controller.close(), {
                            once: true
                        });
                    }
                    break;
                case 'prerender-client':
                case 'prerender-ppr':
                case 'prerender-legacy':
                case 'request':
                case 'cache':
                case 'private-cache':
                case 'unstable-cache':
                case undefined:
                    return controller.close();
                default:
                    workUnitStore;
            }
        }
    }), {
        findSourceMapURL,
        serverConsumerManifest: {
            // moduleLoading must be null because we don't want to trigger preloads of ClientReferences
            // to be added to the current execution. Instead, we'll wait for any ClientReference
            // to be emitted which themselves will handle the preloading.
            moduleLoading: null,
            moduleMap: isEdgeRuntime ? edgeRscModuleMapping : rscModuleMapping,
            serverModuleMap: (0, _manifestssingleton.getServerModuleMap)()
        }
    });
    return deserialized;
}

//# sourceMappingURL=encryption.js.map