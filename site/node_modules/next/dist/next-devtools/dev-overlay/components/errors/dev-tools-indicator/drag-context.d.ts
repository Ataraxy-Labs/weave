import React, { type Ref } from 'react';
interface DragContextValue {
    register: (el: HTMLElement) => void;
    unregister: (el: HTMLElement) => void;
    handles: Set<HTMLElement>;
    disabled: boolean;
}
export declare function DragProvider({ children, disabled, }: {
    children: React.ReactNode;
    disabled?: boolean;
}): import("react/jsx-runtime").JSX.Element;
export declare function useDragContext(): DragContextValue | null;
export declare function DragHandle({ children, ref, ...props }: React.HTMLAttributes<HTMLDivElement> & {
    ref?: Ref<HTMLDivElement>;
}): import("react/jsx-runtime").JSX.Element;
export {};
