import { type HmrMessageSentToBrowser } from '../../../../server/dev/hot-reloader-types';
type MessageCallback = (message: HmrMessageSentToBrowser) => void;
export declare function addMessageListener(callback: MessageCallback): void;
export declare function sendMessage(data: string): void;
export declare function connectHMR(options: {
    path: string;
    assetPrefix: string;
}): void;
export {};
