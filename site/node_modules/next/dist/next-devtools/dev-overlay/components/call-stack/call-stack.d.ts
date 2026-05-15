import type { OriginalStackFrame } from '../../../shared/stack-frame';
export declare function CallStack({ frames, isIgnoreListOpen, ignoredFramesTally, onToggleIgnoreList, }: {
    frames: readonly OriginalStackFrame[];
    isIgnoreListOpen: boolean;
    ignoredFramesTally: number;
    onToggleIgnoreList: () => void;
}): import("react/jsx-runtime").JSX.Element;
export declare const CALL_STACK_STYLES: string;
