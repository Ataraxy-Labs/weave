export declare function useFocusTrap(rootRef: React.RefObject<HTMLElement | null>, triggerRef: React.RefObject<HTMLButtonElement | null> | null, active: boolean, onOpenFocus?: () => void): void;
export declare function getActiveElement(node: HTMLElement | null): HTMLElement | null;
export declare function useClickOutsideAndEscape(rootRef: React.RefObject<HTMLElement | null>, triggerRef: React.RefObject<HTMLButtonElement | null>, active: boolean, close: (reason: 'escape' | 'outside') => void, ownerDocument?: Document): void;
export declare const MENU_DURATION_MS = 200;
export declare const MENU_CURVE = "cubic-bezier(0.175, 0.885, 0.32, 1.1)";
