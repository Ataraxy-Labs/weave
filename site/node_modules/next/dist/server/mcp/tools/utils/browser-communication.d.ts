/**
 * Shared utilities for MCP tools that communicate with the browser.
 * This module provides a common infrastructure for request-response
 * communication between MCP endpoints and browser sessions via HMR.
 */
import type { HMR_MESSAGE_SENT_TO_BROWSER, HmrMessageSentToBrowser } from '../../../dev/hot-reloader-types';
export declare const DEFAULT_BROWSER_REQUEST_TIMEOUT_MS = 5000;
export type BrowserResponse<T> = {
    url: string;
    data: T;
};
export declare function createBrowserRequest<T>(messageType: HMR_MESSAGE_SENT_TO_BROWSER, sendHmrMessage: (message: HmrMessageSentToBrowser) => void, getActiveConnectionCount: () => number, timeoutMs: number): Promise<BrowserResponse<T>[]>;
export declare function handleBrowserPageResponse<T>(requestId: string, data: T, url: string): void;
