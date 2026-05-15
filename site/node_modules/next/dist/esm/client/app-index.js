import { jsx as _jsx } from "react/jsx-runtime";
import './app-globals';
import ReactDOMClient from 'react-dom/client';
import React from 'react';
// TODO: Explicitly import from client.browser
// eslint-disable-next-line import/no-extraneous-dependencies
import { createFromReadableStream as createFromReadableStreamBrowser, createFromFetch as createFromFetchBrowser } from 'react-server-dom-webpack/client';
import { HeadManagerContext } from '../shared/lib/head-manager-context.shared-runtime';
import { onRecoverableError } from './react-client-callbacks/on-recoverable-error';
import { onCaughtError, onUncaughtError } from './react-client-callbacks/error-boundary-callbacks';
import { callServer } from './app-call-server';
import { findSourceMapURL } from './app-find-source-map-url';
import { createMutableActionQueue } from './components/app-router-instance';
import AppRouter from './components/app-router';
import { createInitialRouterState } from './components/router-reducer/create-initial-router-state';
import { MissingSlotContext } from '../shared/lib/app-router-context.shared-runtime';
import { setAppBuildId } from './app-build-id';
import { createInitialRSCPayloadFromFallbackPrerender } from './flight-data-helpers';
/// <reference types="react-dom/experimental" />
const createFromReadableStream = createFromReadableStreamBrowser;
const createFromFetch = createFromFetchBrowser;
const appElement = document;
const encoder = new TextEncoder();
let initialServerDataBuffer = undefined;
let initialServerDataWriter = undefined;
let initialServerDataLoaded = false;
let initialServerDataFlushed = false;
let initialFormStateData = null;
function nextServerDataCallback(seg) {
    if (seg[0] === 0) {
        initialServerDataBuffer = [];
    } else if (seg[0] === 1) {
        if (!initialServerDataBuffer) throw Object.defineProperty(new Error('Unexpected server data: missing bootstrap script.'), "__NEXT_ERROR_CODE", {
            value: "E18",
            enumerable: false,
            configurable: true
        });
        if (initialServerDataWriter) {
            initialServerDataWriter.enqueue(encoder.encode(seg[1]));
        } else {
            initialServerDataBuffer.push(seg[1]);
        }
    } else if (seg[0] === 2) {
        initialFormStateData = seg[1];
    } else if (seg[0] === 3) {
        if (!initialServerDataBuffer) throw Object.defineProperty(new Error('Unexpected server data: missing bootstrap script.'), "__NEXT_ERROR_CODE", {
            value: "E18",
            enumerable: false,
            configurable: true
        });
        // Decode the base64 string back to binary data.
        const binaryString = atob(seg[1]);
        const decodedChunk = new Uint8Array(binaryString.length);
        for(var i = 0; i < binaryString.length; i++){
            decodedChunk[i] = binaryString.charCodeAt(i);
        }
        if (initialServerDataWriter) {
            initialServerDataWriter.enqueue(decodedChunk);
        } else {
            initialServerDataBuffer.push(decodedChunk);
        }
    }
}
function isStreamErrorOrUnfinished(ctr) {
    // If `desiredSize` is null, it means the stream is closed or errored. If it is lower than 0, the stream is still unfinished.
    return ctr.desiredSize === null || ctr.desiredSize < 0;
}
// There might be race conditions between `nextServerDataRegisterWriter` and
// `DOMContentLoaded`. The former will be called when React starts to hydrate
// the root, the latter will be called when the DOM is fully loaded.
// For streaming, the former is called first due to partial hydration.
// For non-streaming, the latter can be called first.
// Hence, we use two variables `initialServerDataLoaded` and
// `initialServerDataFlushed` to make sure the writer will be closed and
// `initialServerDataBuffer` will be cleared in the right time.
function nextServerDataRegisterWriter(ctr) {
    if (initialServerDataBuffer) {
        initialServerDataBuffer.forEach((val)=>{
            ctr.enqueue(typeof val === 'string' ? encoder.encode(val) : val);
        });
        if (initialServerDataLoaded && !initialServerDataFlushed) {
            if (isStreamErrorOrUnfinished(ctr)) {
                ctr.error(Object.defineProperty(new Error('The connection to the page was unexpectedly closed, possibly due to the stop button being clicked, loss of Wi-Fi, or an unstable internet connection.'), "__NEXT_ERROR_CODE", {
                    value: "E117",
                    enumerable: false,
                    configurable: true
                }));
            } else {
                ctr.close();
            }
            initialServerDataFlushed = true;
            initialServerDataBuffer = undefined;
        }
    }
    initialServerDataWriter = ctr;
}
// When `DOMContentLoaded`, we can close all pending writers to finish hydration.
const DOMContentLoaded = function() {
    if (initialServerDataWriter && !initialServerDataFlushed) {
        initialServerDataWriter.close();
        initialServerDataFlushed = true;
        initialServerDataBuffer = undefined;
    }
    initialServerDataLoaded = true;
};
// It's possible that the DOM is already loaded.
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', DOMContentLoaded, false);
} else {
    // Delayed in marco task to ensure it's executed later than hydration
    setTimeout(DOMContentLoaded);
}
const nextServerDataLoadingGlobal = self.__next_f = self.__next_f || [];
// Consume all buffered chunks and clear the global data array right after to release memory.
// Otherwise it will be retained indefinitely.
nextServerDataLoadingGlobal.forEach(nextServerDataCallback);
nextServerDataLoadingGlobal.length = 0;
// Patch its push method so subsequent chunks are handled (but not actually pushed to the array).
nextServerDataLoadingGlobal.push = nextServerDataCallback;
const readable = new ReadableStream({
    start (controller) {
        nextServerDataRegisterWriter(controller);
    }
});
if (process.env.NODE_ENV !== 'production') {
    // @ts-expect-error
    readable.name = 'hydration';
}
let debugChannel;
if (process.env.NODE_ENV !== 'production' && process.env.__NEXT_REACT_DEBUG_CHANNEL && typeof window !== 'undefined') {
    const { createDebugChannel } = require('./dev/debug-channel');
    debugChannel = createDebugChannel(undefined);
}
const clientResumeFetch = // @ts-expect-error
window.__NEXT_CLIENT_RESUME;
let initialServerResponse;
if (clientResumeFetch) {
    initialServerResponse = Promise.resolve(createFromFetch(clientResumeFetch, {
        callServer,
        findSourceMapURL,
        debugChannel
    })).then(async (fallbackInitialRSCPayload)=>createInitialRSCPayloadFromFallbackPrerender(await clientResumeFetch, fallbackInitialRSCPayload));
} else {
    initialServerResponse = createFromReadableStream(readable, {
        callServer,
        findSourceMapURL,
        debugChannel,
        startTime: 0
    });
}
function ServerRoot({ initialRSCPayload, actionQueue, webSocket, staticIndicatorState }) {
    const router = /*#__PURE__*/ _jsx(AppRouter, {
        actionQueue: actionQueue,
        globalErrorState: initialRSCPayload.G,
        webSocket: webSocket,
        staticIndicatorState: staticIndicatorState
    });
    if (process.env.NODE_ENV === 'development' && initialRSCPayload.m) {
        // We provide missing slot information in a context provider only during development
        // as we log some additional information about the missing slots in the console.
        return /*#__PURE__*/ _jsx(MissingSlotContext, {
            value: initialRSCPayload.m,
            children: router
        });
    }
    return router;
}
const StrictModeIfEnabled = process.env.__NEXT_STRICT_MODE_APP ? React.StrictMode : React.Fragment;
function Root({ children }) {
    if (process.env.__NEXT_TEST_MODE) {
        // eslint-disable-next-line react-hooks/rules-of-hooks
        React.useEffect(()=>{
            window.__NEXT_HYDRATED = true;
            window.__NEXT_HYDRATED_AT = performance.now();
            window.__NEXT_HYDRATED_CB?.();
        }, []);
    }
    return children;
}
const enableTransitionIndicator = process.env.__NEXT_TRANSITION_INDICATOR;
function noDefaultTransitionIndicator() {
    return ()=>{};
}
const reactRootOptions = {
    onDefaultTransitionIndicator: enableTransitionIndicator ? undefined : noDefaultTransitionIndicator,
    onRecoverableError,
    onCaughtError,
    onUncaughtError
};
export async function hydrate(instrumentationHooks, assetPrefix) {
    let staticIndicatorState;
    let webSocket;
    if (process.env.NODE_ENV !== 'production') {
        const { createWebSocket } = require('./dev/hot-reloader/app/web-socket');
        staticIndicatorState = {
            pathname: null,
            appIsrManifest: null
        };
        webSocket = createWebSocket(assetPrefix, staticIndicatorState);
    }
    const initialRSCPayload = await initialServerResponse;
    // setAppBuildId should be called only once, during JS initialization
    // and before any components have hydrated.
    setAppBuildId(initialRSCPayload.b);
    const initialTimestamp = Date.now();
    const actionQueue = createMutableActionQueue(createInitialRouterState({
        navigatedAt: initialTimestamp,
        initialFlightData: initialRSCPayload.f,
        initialCanonicalUrlParts: initialRSCPayload.c,
        initialRenderedSearch: initialRSCPayload.q,
        location: window.location
    }), instrumentationHooks);
    const reactEl = /*#__PURE__*/ _jsx(StrictModeIfEnabled, {
        children: /*#__PURE__*/ _jsx(HeadManagerContext.Provider, {
            value: {
                appDir: true
            },
            children: /*#__PURE__*/ _jsx(Root, {
                children: /*#__PURE__*/ _jsx(ServerRoot, {
                    initialRSCPayload: initialRSCPayload,
                    actionQueue: actionQueue,
                    webSocket: webSocket,
                    staticIndicatorState: staticIndicatorState
                })
            })
        })
    });
    if (document.documentElement.id === '__next_error__') {
        let element = reactEl;
        // Server rendering failed, fall back to client-side rendering
        if (process.env.NODE_ENV !== 'production') {
            const { RootLevelDevOverlayElement } = require('../next-devtools/userspace/app/client-entry');
            // Note this won't cause hydration mismatch because we are doing CSR w/o hydration
            element = /*#__PURE__*/ _jsx(RootLevelDevOverlayElement, {
                children: element
            });
        }
        ReactDOMClient.createRoot(appElement, reactRootOptions).render(element);
    } else {
        React.startTransition(()=>{
            ReactDOMClient.hydrateRoot(appElement, reactEl, {
                ...reactRootOptions,
                formState: initialFormStateData
            });
        });
    }
    // TODO-APP: Remove this logic when Float has GC built-in in development.
    if (process.env.NODE_ENV !== 'production') {
        const { linkGc } = require('./app-link-gc');
        linkGc();
    }
}

//# sourceMappingURL=app-index.js.map