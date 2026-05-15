import type { DevToolsIndicatorPosition, DevToolsScale } from '../../../../shared';
export declare function UserPreferencesBody({ theme, hide, hideShortcut, setHideShortcut, scale, setPosition, setScale, position, }: {
    theme: 'dark' | 'light' | 'system';
    hide: () => void;
    hideShortcut: string | null;
    setHideShortcut: (value: string | null) => void;
    setPosition: (position: DevToolsIndicatorPosition) => void;
    position: DevToolsIndicatorPosition;
    scale: DevToolsScale;
    setScale: (value: DevToolsScale) => void;
}): import("react/jsx-runtime").JSX.Element;
export declare const DEV_TOOLS_INFO_USER_PREFERENCES_STYLES: string;
