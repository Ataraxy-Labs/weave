import { htmlEscapeJsonString } from '../htmlescape';
import { workUnitAsyncStorage } from './work-unit-async-storage.external';
import { InvariantError } from '../../shared/lib/invariant-error';
import { getClientReferenceManifest } from './manifests-singleton';
const isEdgeRuntime = process.env.NEXT_RUNTIME === 'edge';
const INLINE_FLIGHT_PAYLOAD_BOOTSTRAP = 0;
const INLINE_FLIGHT_PAYLOAD_DATA = 1;
const INLINE_FLIGHT_PAYLOAD_FORM_STATE = 2;
const INLINE_FLIGHT_PAYLOAD_BINARY = 3;
const flightResponses = new WeakMap();
const encoder = new TextEncoder();
const findSourceMapURL = process.env.NODE_ENV !== 'production' ? require('../lib/source-maps').findSourceMapURLDEV : undefined;
/**
 * Render Flight stream.
 * This is only used for renderToHTML, the Flight response does not need additional wrappers.
 */ export function getFlightStream(flightStream, debugStream, debugEndTime, nonce) {
    const response = flightResponses.get(flightStream);
    if (response) {
        return response;
    }
    const { moduleLoading, edgeSSRModuleMapping, ssrModuleMapping } = getClientReferenceManifest();
    let newResponse;
    if (flightStream instanceof ReadableStream) {
        // The types of flightStream and debugStream should match.
        if (debugStream && !(debugStream instanceof ReadableStream)) {
            throw Object.defineProperty(new InvariantError('Expected debug stream to be a ReadableStream'), "__NEXT_ERROR_CODE", {
                value: "E939",
                enumerable: false,
                configurable: true
            });
        }
        // react-server-dom-webpack/client.edge must not be hoisted for require cache clearing to work correctly
        const { createFromReadableStream } = // eslint-disable-next-line import/no-extraneous-dependencies
        require('react-server-dom-webpack/client');
        newResponse = createFromReadableStream(flightStream, {
            findSourceMapURL,
            serverConsumerManifest: {
                moduleLoading,
                moduleMap: isEdgeRuntime ? edgeSSRModuleMapping : ssrModuleMapping,
                serverModuleMap: null
            },
            nonce,
            debugChannel: debugStream ? {
                readable: debugStream
            } : undefined,
            endTime: debugEndTime
        });
    } else {
        if (process.env.NEXT_RUNTIME === 'edge') {
            throw Object.defineProperty(new InvariantError('getFlightStream should always receive a ReadableStream when using the edge runtime'), "__NEXT_ERROR_CODE", {
                value: "E943",
                enumerable: false,
                configurable: true
            });
        } else {
            const { Readable } = require('node:stream');
            // The types of flightStream and debugStream should match.
            if (debugStream && !(debugStream instanceof Readable)) {
                throw Object.defineProperty(new InvariantError('Expected debug stream to be a Readable'), "__NEXT_ERROR_CODE", {
                    value: "E940",
                    enumerable: false,
                    configurable: true
                });
            }
            // react-server-dom-webpack/client.edge must not be hoisted for require cache clearing to work correctly
            const { createFromNodeStream } = // eslint-disable-next-line import/no-extraneous-dependencies
            require('react-server-dom-webpack/client');
            newResponse = createFromNodeStream(flightStream, {
                moduleLoading,
                moduleMap: isEdgeRuntime ? edgeSSRModuleMapping : ssrModuleMapping,
                serverModuleMap: null
            }, {
                findSourceMapURL,
                nonce,
                debugChannel: debugStream,
                endTime: debugEndTime
            });
        }
    }
    // Edge pages are never prerendered so they necessarily cannot have a workUnitStore type
    // that requires the nextTick behavior. This is why it is safe to access a node only API here
    if (process.env.NEXT_RUNTIME !== 'edge') {
        const workUnitStore = workUnitAsyncStorage.getStore();
        if (!workUnitStore) {
            throw Object.defineProperty(new InvariantError('Expected workUnitAsyncStorage to have a store.'), "__NEXT_ERROR_CODE", {
                value: "E696",
                enumerable: false,
                configurable: true
            });
        }
        switch(workUnitStore.type){
            case 'prerender-client':
                const responseOnNextTick = new Promise((resolve)=>{
                    process.nextTick(()=>{
                        resolve(newResponse);
                    });
                });
                flightResponses.set(flightStream, responseOnNextTick);
                return responseOnNextTick;
            case 'prerender':
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
    flightResponses.set(flightStream, newResponse);
    return newResponse;
}
/**
 * Creates a ReadableStream provides inline script tag chunks for writing hydration
 * data to the client outside the React render itself.
 *
 * @param flightStream The RSC render stream
 * @param nonce optionally a nonce used during this particular render
 * @param formState optionally the formState used with this particular render
 * @returns a ReadableStream without the complete property. This signifies a lazy ReadableStream
 */ export function createInlinedDataReadableStream(flightStream, nonce, formState) {
    const startScriptTag = nonce ? `<script nonce=${JSON.stringify(nonce)}>` : '<script>';
    const flightReader = flightStream.getReader();
    const decoder = new TextDecoder('utf-8', {
        fatal: true
    });
    const readable = new ReadableStream({
        type: 'bytes',
        start (controller) {
            try {
                writeInitialInstructions(controller, startScriptTag, formState);
            } catch (error) {
                // during encoding or enqueueing forward the error downstream
                controller.error(error);
            }
        },
        async pull (controller) {
            try {
                const { done, value } = await flightReader.read();
                if (value) {
                    try {
                        const decodedString = decoder.decode(value, {
                            stream: !done
                        });
                        // The chunk cannot be decoded as valid UTF-8 string as it might
                        // have arbitrary binary data.
                        writeFlightDataInstruction(controller, startScriptTag, decodedString);
                    } catch  {
                        // The chunk cannot be decoded as valid UTF-8 string.
                        writeFlightDataInstruction(controller, startScriptTag, value);
                    }
                }
                if (done) {
                    controller.close();
                }
            } catch (error) {
                // There was a problem in the upstream reader or during decoding or enqueuing
                // forward the error downstream
                controller.error(error);
            }
        }
    });
    return readable;
}
function writeInitialInstructions(controller, scriptStart, formState) {
    let scriptContents = `(self.__next_f=self.__next_f||[]).push(${htmlEscapeJsonString(JSON.stringify([
        INLINE_FLIGHT_PAYLOAD_BOOTSTRAP
    ]))})`;
    if (formState != null) {
        scriptContents += `;self.__next_f.push(${htmlEscapeJsonString(JSON.stringify([
            INLINE_FLIGHT_PAYLOAD_FORM_STATE,
            formState
        ]))})`;
    }
    controller.enqueue(encoder.encode(`${scriptStart}${scriptContents}</script>`));
}
function writeFlightDataInstruction(controller, scriptStart, chunk) {
    let htmlInlinedData;
    if (typeof chunk === 'string') {
        htmlInlinedData = htmlEscapeJsonString(JSON.stringify([
            INLINE_FLIGHT_PAYLOAD_DATA,
            chunk
        ]));
    } else {
        // The chunk cannot be embedded as a UTF-8 string in the script tag.
        // Instead let's inline it in base64.
        // Credits to Devon Govett (devongovett) for the technique.
        // https://github.com/devongovett/rsc-html-stream
        const base64 = btoa(String.fromCodePoint(...chunk));
        htmlInlinedData = htmlEscapeJsonString(JSON.stringify([
            INLINE_FLIGHT_PAYLOAD_BINARY,
            base64
        ]));
    }
    controller.enqueue(encoder.encode(`${scriptStart}self.__next_f.push(${htmlInlinedData})</script>`));
}

//# sourceMappingURL=use-flight-response.js.map