import type { CacheIndicatorState } from '../../cache-indicator';
export declare enum Status {
    None = "none",
    Rendering = "rendering",
    Compiling = "compiling",
    Prerendering = "prerendering",
    CacheBypassing = "cache-bypassing"
}
export declare function getCurrentStatus(buildingIndicator: boolean, renderingIndicator: boolean, cacheIndicator: CacheIndicatorState): Status;
interface StatusIndicatorProps {
    status: Status;
    onClick?: () => void;
}
export declare function StatusIndicator({ status, onClick }: StatusIndicatorProps): import("react/jsx-runtime").JSX.Element | null;
export {};
