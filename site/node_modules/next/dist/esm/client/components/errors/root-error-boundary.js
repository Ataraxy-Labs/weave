'use client';
import { jsx as _jsx } from "react/jsx-runtime";
import React from 'react';
import GracefulDegradeBoundary from './graceful-degrade-boundary';
import { ErrorBoundary } from '../error-boundary';
import { isBot } from '../../../shared/lib/router/utils/is-bot';
const isBotUserAgent = typeof window !== 'undefined' && isBot(window.navigator.userAgent);
export default function RootErrorBoundary({ children, errorComponent, errorStyles, errorScripts }) {
    if (isBotUserAgent) {
        // Preserve existing DOM/HTML for bots to avoid replacing content with an error UI
        // and to keep the original SSR output intact.
        return /*#__PURE__*/ _jsx(GracefulDegradeBoundary, {
            children: children
        });
    }
    return /*#__PURE__*/ _jsx(ErrorBoundary, {
        errorComponent: errorComponent,
        errorStyles: errorStyles,
        errorScripts: errorScripts,
        children: children
    });
}

//# sourceMappingURL=root-error-boundary.js.map