import type { HmrMessageSentToBrowser } from '../../../server/dev/hot-reloader-types';
export declare const REACT_REFRESH_FULL_RELOAD: string;
export declare const REACT_REFRESH_FULL_RELOAD_FROM_ERROR = "[Fast Refresh] performing full reload because your application had an unrecoverable error";
export declare function reportInvalidHmrMessage(message: HmrMessageSentToBrowser | MessageEvent<unknown>, err: unknown): void;
