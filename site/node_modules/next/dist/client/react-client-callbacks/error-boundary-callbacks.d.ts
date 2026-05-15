import type { ErrorInfo } from 'react';
export declare function onCaughtError(thrownValue: unknown, errorInfo: ErrorInfo & {
    errorBoundary?: React.Component;
}): void;
export declare function onUncaughtError(thrownValue: unknown): void;
