import { type OverlayDispatch, type OverlayState } from '../../../shared';
import type { ReadyRuntimeError } from '../../../utils/get-error-by-type';
import type { HydrationErrorState } from '../../../../shared/hydration-error';
export interface ErrorBaseProps {
    rendered: boolean;
    transitionDurationMs: number;
    isTurbopack: boolean;
    versionInfo: OverlayState['versionInfo'];
    errorCount: number;
}
export declare function ErrorOverlay({ state, dispatch, getSquashedHydrationErrorDetails, runtimeErrors, errorCount, }: {
    state: OverlayState;
    dispatch: OverlayDispatch;
    getSquashedHydrationErrorDetails: (error: Error) => HydrationErrorState | null;
    runtimeErrors: ReadyRuntimeError[];
    errorCount: number;
}): import("react/jsx-runtime").JSX.Element;
