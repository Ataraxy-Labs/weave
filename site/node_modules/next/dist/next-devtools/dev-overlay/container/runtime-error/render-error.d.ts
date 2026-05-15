import type { OverlayState } from '../../shared';
import type { StackFrame } from '../../../shared/stack-frame';
import { type ReadyRuntimeError } from '../../utils/get-error-by-type';
export type SupportedErrorEvent = {
    id: number;
    error: Error;
    frames: readonly StackFrame[];
    type: 'runtime' | 'recoverable' | 'console';
};
type Props = {
    children: (params: {
        runtimeErrors: ReadyRuntimeError[];
        totalErrorCount: number;
    }) => React.ReactNode;
    state: OverlayState;
    isAppDir: boolean;
};
export declare const RenderError: (props: Props) => import("react/jsx-runtime").JSX.Element;
export {};
