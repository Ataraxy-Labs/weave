import './devtools-indicator.css';
import type { DevToolsIndicatorPosition } from '../../shared';
export declare const INDICATOR_PADDING = 20;
export declare function DevToolsIndicator(): import("react/jsx-runtime").JSX.Element;
/**
 * makes sure we eventually sync the panel to the logo, otherwise
 * it will be jarring if the panels start appearing on the other
 * side of the logo. This wont teleport the panel because the indicator
 * cannot be dragged when any panel is open
 */
export declare const useUpdateAllPanelPositions: () => (position: DevToolsIndicatorPosition) => void;
