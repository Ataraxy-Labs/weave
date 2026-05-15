import type { ReactNode } from 'react';
import type { HmrMessageSentToBrowser, TurbopackMessageSentToBrowser } from '../../../../server/dev/hot-reloader-types';
import { type GlobalErrorState } from '../../../components/app-router-instance';
export interface StaticIndicatorState {
    pathname: string | null;
    appIsrManifest: Record<string, boolean> | null;
}
export declare function waitForWebpackRuntimeHotUpdate(): Promise<void>;
export declare function performFullReload(err: any, sendMessage: (data: string) => void): void;
/** Handles messages from the server for the App Router. */
export declare function processMessage(message: HmrMessageSentToBrowser, sendMessage: (message: string) => void, processTurbopackMessage: (msg: TurbopackMessageSentToBrowser) => void, staticIndicatorState: StaticIndicatorState): void;
export default function HotReload({ children, globalError, webSocket, staticIndicatorState, }: {
    children: ReactNode;
    globalError: GlobalErrorState;
    webSocket: WebSocket | undefined;
    staticIndicatorState: StaticIndicatorState | undefined;
}): import("react/jsx-runtime").JSX.Element;
