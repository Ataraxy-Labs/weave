import { jsx as _jsx } from "react/jsx-runtime";
import React from 'react';
import DefaultGlobalError from '../../../client/components/builtin/global-error';
import { AppDevOverlayErrorBoundary } from './app-dev-overlay-error-boundary';
// If an error is thrown while rendering an RSC stream, this will catch it in
// dev and show the error overlay.
export function RootLevelDevOverlayElement({ children }) {
    return /*#__PURE__*/ _jsx(AppDevOverlayErrorBoundary, {
        globalError: [
            DefaultGlobalError,
            null
        ],
        children: children
    });
}

//# sourceMappingURL=client-entry.js.map