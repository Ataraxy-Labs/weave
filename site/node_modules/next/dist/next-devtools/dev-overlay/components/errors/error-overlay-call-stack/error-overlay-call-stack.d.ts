import type { OriginalStackFrame } from '../../../../shared/stack-frame';
interface CallStackProps {
    frames: readonly OriginalStackFrame[];
    dialogResizerRef: React.RefObject<HTMLDivElement | null>;
}
export declare function ErrorOverlayCallStack({ frames, dialogResizerRef, }: CallStackProps): import("react/jsx-runtime").JSX.Element;
export {};
