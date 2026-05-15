import { type BinaryHmrMessageSentToBrowser } from './hot-reloader-types';
export declare const FAST_REFRESH_RUNTIME_RELOAD = "Fast Refresh had to perform a full reload due to a runtime error.";
export declare function createBinaryHmrMessageData(message: BinaryHmrMessageSentToBrowser): Uint8Array;
