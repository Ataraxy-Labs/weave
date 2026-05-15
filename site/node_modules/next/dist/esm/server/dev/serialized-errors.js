import { streamToUint8Array } from '../stream-utils/node-web-streams-helper';
import { HMR_MESSAGE_SENT_TO_BROWSER } from './hot-reloader-types';
const errorsRscStreamsByHtmlRequestId = new Map();
export function sendSerializedErrorsToClient(errorsRscStream, sendToClient) {
    streamToUint8Array(errorsRscStream).then((serializedErrors)=>{
        sendToClient({
            type: HMR_MESSAGE_SENT_TO_BROWSER.ERRORS_TO_SHOW_IN_BROWSER,
            serializedErrors
        });
    }, (err)=>{
        console.error(Object.defineProperty(new Error('Failed to serialize errors.', {
            cause: err
        }), "__NEXT_ERROR_CODE", {
            value: "E948",
            enumerable: false,
            configurable: true
        }));
    });
}
export function sendSerializedErrorsToClientForHtmlRequest(htmlRequestId, sendToClient) {
    const errorsRscStream = errorsRscStreamsByHtmlRequestId.get(htmlRequestId);
    if (!errorsRscStream) {
        return;
    }
    errorsRscStreamsByHtmlRequestId.delete(htmlRequestId);
    sendSerializedErrorsToClient(errorsRscStream, sendToClient);
}
export function setErrorsRscStreamForHtmlRequest(htmlRequestId, errorsRscStream) {
    // TODO: Clean up after a timeout, in case the client never connects, e.g.
    // when CURL'ing the page, or loading the page with JavaScript disabled etc.
    errorsRscStreamsByHtmlRequestId.set(htmlRequestId, errorsRscStream);
}
export function deleteErrorsRscStreamForHtmlRequest(htmlRequestId) {
    errorsRscStreamsByHtmlRequestId.delete(htmlRequestId);
}

//# sourceMappingURL=serialized-errors.js.map