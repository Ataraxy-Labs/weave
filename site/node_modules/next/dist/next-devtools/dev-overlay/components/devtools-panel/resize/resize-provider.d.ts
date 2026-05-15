import { type RefObject } from 'react';
import { type Corners } from '../../../shared';
export type ResizeDirection = 'top' | 'right' | 'bottom' | 'left' | 'top-left' | 'top-right' | 'bottom-left' | 'bottom-right';
interface ResizeContextValue {
    resizeRef: RefObject<HTMLElement | null>;
    minWidth: number;
    minHeight: number;
    maxWidth?: number;
    maxHeight?: number;
    draggingDirection: ResizeDirection | null;
    setDraggingDirection: (direction: ResizeDirection | null) => void;
    storageKey: string;
}
interface ResizeProviderProps {
    value: {
        resizeRef: RefObject<HTMLElement | null>;
        minWidth?: number;
        minHeight?: number;
        maxWidth?: number;
        maxHeight?: number;
        devToolsPosition: Corners;
        devToolsPanelSize: Record<string, {
            width: number;
            height: number;
        }>;
        storageKey?: string;
        initialSize?: {
            height: number;
            width: number;
        };
    };
    children: React.ReactNode;
}
export declare const ResizeProvider: ({ value, children }: ResizeProviderProps) => import("react/jsx-runtime").JSX.Element;
export declare const useResize: () => ResizeContextValue;
export {};
