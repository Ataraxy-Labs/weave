import { type HmrMessageSentToBrowser } from './hot-reloader-types';
export declare function sendSerializedErrorsToClient(errorsRscStream: ReadableStream<Uint8Array>, sendToClient: (message: HmrMessageSentToBrowser) => void): void;
export declare function sendSerializedErrorsToClientForHtmlRequest(htmlRequestId: string, sendToClient: (message: HmrMessageSentToBrowser) => void): void;
export declare function setErrorsRscStreamForHtmlRequest(htmlRequestId: string, errorsRscStream: ReadableStream<Uint8Array>): void;
export declare function deleteErrorsRscStreamForHtmlRequest(htmlRequestId: string): void;
