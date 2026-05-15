import * as React from 'react';
import type { DebugInfo } from '../../../../shared/types';
import type { ErrorMessageType } from '../error-message/error-message';
import type { ErrorType } from '../error-type-label/error-type-label';
import type { ErrorBaseProps } from '../error-overlay/error-overlay';
import type { ReadyRuntimeError } from '../../../utils/get-error-by-type';
export interface ErrorOverlayLayoutProps extends ErrorBaseProps {
    errorMessage: ErrorMessageType;
    errorType: ErrorType;
    children?: React.ReactNode;
    errorCode?: string;
    error: ReadyRuntimeError['error'];
    debugInfo?: DebugInfo;
    isBuildError?: boolean;
    onClose?: () => void;
    runtimeErrors?: ReadyRuntimeError[];
    activeIdx?: number;
    setActiveIndex?: (index: number) => void;
    dialogResizerRef?: React.RefObject<HTMLDivElement | null>;
    generateErrorInfo: () => string;
}
export declare function ErrorOverlayLayout({ errorMessage, errorType, children, errorCode, errorCount, error, debugInfo, isBuildError, onClose, versionInfo, runtimeErrors, activeIdx, setActiveIndex, isTurbopack, dialogResizerRef, generateErrorInfo, rendered, transitionDurationMs, }: ErrorOverlayLayoutProps): import("react/jsx-runtime").JSX.Element;
export declare const styles: string;
