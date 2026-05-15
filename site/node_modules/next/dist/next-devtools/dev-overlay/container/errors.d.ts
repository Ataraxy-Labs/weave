import type { DebugInfo } from '../../shared/types';
import { type ErrorOverlayLayoutProps } from '../components/errors/error-overlay-layout/error-overlay-layout';
import type { ReadyRuntimeError } from '../utils/get-error-by-type';
import type { ErrorBaseProps } from '../components/errors/error-overlay/error-overlay';
import type { HydrationErrorState } from '../../shared/hydration-error';
interface ErrorsProps extends ErrorBaseProps {
    getSquashedHydrationErrorDetails: (error: Error) => HydrationErrorState | null;
    runtimeErrors: ReadyRuntimeError[];
    debugInfo: DebugInfo;
    onClose: () => void;
}
export declare function getErrorTypeLabel(error: Error, type: ReadyRuntimeError['type'], errorDetails: ErrorDetails): ErrorOverlayLayoutProps['errorType'];
type ErrorDetails = NoErrorDetails | HydrationErrorDetails | BlockingRouteErrorDetails | DynamicMetadataErrorDetails;
type NoErrorDetails = {
    type: 'empty';
};
type HydrationErrorDetails = {
    type: 'hydration';
    warning: string | null;
    notes: string | null;
    reactOutputComponentDiff: string | null;
};
type BlockingRouteErrorDetails = {
    type: 'blocking-route';
    variant: 'navigation' | 'runtime';
    refinement: '' | 'generateViewport';
};
type DynamicMetadataErrorDetails = {
    type: 'dynamic-metadata';
    variant: 'navigation' | 'runtime';
};
export declare function useErrorDetails(error: Error | undefined, getSquashedHydrationErrorDetails: (error: Error) => HydrationErrorState | null): ErrorDetails;
export declare function Errors({ getSquashedHydrationErrorDetails, runtimeErrors, debugInfo, onClose, ...props }: ErrorsProps): import("react/jsx-runtime").JSX.Element | null;
export declare const styles = "\n  .nextjs-error-with-static {\n    bottom: calc(16px * 4.5);\n  }\n  p.nextjs__container_errors__link {\n    font-size: var(--size-14);\n  }\n  p.nextjs__container_errors__notes {\n    color: var(--color-stack-notes);\n    font-size: var(--size-14);\n    line-height: 1.5;\n  }\n  .nextjs-container-errors-body > h2:not(:first-child) {\n    margin-top: calc(16px + 8px);\n  }\n  .nextjs-container-errors-body > h2 {\n    color: var(--color-title-color);\n    margin-bottom: 8px;\n    font-size: var(--size-20);\n  }\n  .nextjs-toast-errors-parent {\n    cursor: pointer;\n    transition: transform 0.2s ease;\n  }\n  .nextjs-toast-errors-parent:hover {\n    transform: scale(1.1);\n  }\n  .nextjs-toast-errors {\n    display: flex;\n    align-items: center;\n    justify-content: flex-start;\n  }\n  .nextjs-toast-errors > svg {\n    margin-right: 8px;\n  }\n  .nextjs-toast-hide-button {\n    margin-left: 24px;\n    border: none;\n    background: none;\n    color: var(--color-ansi-bright-white);\n    padding: 0;\n    transition: opacity 0.25s ease;\n    opacity: 0.7;\n  }\n  .nextjs-toast-hide-button:hover {\n    opacity: 1;\n  }\n  .nextjs__container_errors__error_title {\n    display: flex;\n    align-items: center;\n    justify-content: space-between;\n    margin-bottom: 14px;\n  }\n  .error-overlay-notes-container {\n    margin: 8px 2px;\n  }\n  .error-overlay-notes-container p {\n    white-space: pre-wrap;\n  }\n  .nextjs__blocking_page_load_error_description {\n    color: var(--color-stack-notes);\n  }\n  .nextjs__blocking_page_load_error_description_title {\n    color: var(--color-title-color);\n  }\n  .nextjs__blocking_page_load_error_fix_option {\n    background-color: var(--color-background-200);\n    padding: 14px;\n    border-radius: var(--rounded-md-2);\n    border: 1px solid var(--color-gray-alpha-400);\n  }\n  .external-link, .external-link:hover {\n    color:inherit;\n  }\n";
export {};
