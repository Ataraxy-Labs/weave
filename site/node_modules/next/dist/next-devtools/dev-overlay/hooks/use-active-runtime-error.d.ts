import type { ReadyRuntimeError } from '../utils/get-error-by-type';
import type { HydrationErrorState } from '../../shared/hydration-error';
export declare function useActiveRuntimeError({ runtimeErrors, getSquashedHydrationErrorDetails, }: {
    runtimeErrors: ReadyRuntimeError[];
    getSquashedHydrationErrorDetails: (error: Error) => HydrationErrorState | null;
}): {
    isLoading: boolean;
    activeIdx: number;
    setActiveIndex: import("react").Dispatch<import("react").SetStateAction<number>>;
    activeError: null;
    errorDetails: null;
    errorCode: null;
    errorType: null;
} | {
    isLoading: false;
    activeIdx: number;
    setActiveIndex: import("react").Dispatch<import("react").SetStateAction<number>>;
    activeError: ReadyRuntimeError;
    errorDetails: {
        type: "empty";
    } | {
        type: "hydration";
        warning: string | null;
        notes: string | null;
        reactOutputComponentDiff: string | null;
    } | {
        type: "blocking-route";
        variant: "navigation" | "runtime";
        refinement: "" | "generateViewport";
    } | {
        type: "dynamic-metadata";
        variant: "navigation" | "runtime";
    };
    errorCode: string | undefined;
    errorType: import("../components/errors/error-type-label/error-type-label").ErrorType;
};
