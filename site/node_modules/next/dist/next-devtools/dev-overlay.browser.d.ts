import { type OverlayState, type DispatcherEvent } from './dev-overlay/shared';
import { type ActionDispatch } from 'react';
import type { CacheIndicatorState } from './dev-overlay/cache-indicator';
import type { HydrationErrorState } from './shared/hydration-error';
import type { DebugInfo } from './shared/types';
import type { DevIndicatorServerState } from '../server/dev/dev-indicator-server-state';
import type { VersionInfo } from '../server/dev/parse-version-info';
import type { SegmentNodeState } from './userspace/app/segment-explorer-node';
import type { DevToolsConfig } from './dev-overlay/shared';
import type { SegmentTrieData } from '../shared/lib/mcp-page-metadata-types';
export interface Dispatcher {
    onBuildOk(): void;
    onBuildError(message: string): void;
    onVersionInfo(versionInfo: VersionInfo): void;
    onDebugInfo(debugInfo: DebugInfo): void;
    onBeforeRefresh(): void;
    onRefresh(): void;
    onCacheIndicator(status: CacheIndicatorState): void;
    onStaticIndicator(status: 'pending' | 'static' | 'dynamic' | 'disabled'): void;
    onDevIndicator(devIndicator: DevIndicatorServerState): void;
    onDevToolsConfig(config: DevToolsConfig): void;
    onUnhandledError(reason: Error): void;
    onUnhandledRejection(reason: Error): void;
    openErrorOverlay(): void;
    closeErrorOverlay(): void;
    toggleErrorOverlay(): void;
    buildingIndicatorHide(): void;
    buildingIndicatorShow(): void;
    renderingIndicatorHide(): void;
    renderingIndicatorShow(): void;
    segmentExplorerNodeAdd(nodeState: SegmentNodeState): void;
    segmentExplorerNodeRemove(nodeState: SegmentNodeState): void;
    segmentExplorerUpdateRouteState(page: string): void;
}
type OverlayStateWithRouter = OverlayState & {
    routerType: 'pages' | 'app';
};
export declare function getSerializedOverlayState(): OverlayStateWithRouter | null;
export declare function getSegmentTrieData(): SegmentTrieData | null;
export declare const dispatcher: Dispatcher;
export declare const DevOverlayContext: import("react").Context<{
    shadowRoot: ShadowRoot;
    state: OverlayState & {
        routerType: "pages" | "app";
    };
    dispatch: ActionDispatch<[action: DispatcherEvent]>;
    getSquashedHydrationErrorDetails: (error: Error) => HydrationErrorState | null;
}>;
export declare const useDevOverlayContext: () => {
    shadowRoot: ShadowRoot;
    state: OverlayState & {
        routerType: "pages" | "app";
    };
    dispatch: ActionDispatch<[action: DispatcherEvent]>;
    getSquashedHydrationErrorDetails: (error: Error) => HydrationErrorState | null;
};
export declare function renderAppDevOverlay(getOwnerStack: (error: Error) => string | null | undefined, isRecoverableError: (error: Error) => boolean, enableCacheIndicator: boolean): void;
export declare function renderPagesDevOverlay(getOwnerStack: (error: Error) => string | null | undefined, getSquashedHydrationErrorDetails: (error: Error) => HydrationErrorState | null, isRecoverableError: (error: Error) => boolean): void;
export {};
