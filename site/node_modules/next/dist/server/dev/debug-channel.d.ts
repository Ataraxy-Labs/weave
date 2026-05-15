import { type HmrMessageSentToBrowser } from './hot-reloader-types';
export interface ReactDebugChannelForBrowser {
    readonly readable: ReadableStream<Uint8Array>;
}
export declare function connectReactDebugChannel(requestId: string, debugChannel: ReactDebugChannelForBrowser, sendToClient: (message: HmrMessageSentToBrowser) => void): void;
export declare function connectReactDebugChannelForHtmlRequest(htmlRequestId: string, sendToClient: (message: HmrMessageSentToBrowser) => void): void;
export declare function setReactDebugChannelForHtmlRequest(htmlRequestId: string, debugChannel: ReactDebugChannelForBrowser): void;
export declare function deleteReactDebugChannelForHtmlRequest(htmlRequestId: string): void;
