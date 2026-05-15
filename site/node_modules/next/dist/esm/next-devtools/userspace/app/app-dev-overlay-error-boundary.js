import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { PureComponent } from 'react';
import { dispatcher } from 'next/dist/compiled/next-devtools';
import { RuntimeErrorHandler } from '../../../client/dev/runtime-error-handler';
import { ErrorBoundary } from '../../../client/components/error-boundary';
import DefaultGlobalError from '../../../client/components/builtin/global-error';
import { SEGMENT_EXPLORER_SIMULATED_ERROR_MESSAGE } from './segment-explorer-node';
function ErroredHtml({ globalError: [GlobalError, globalErrorStyles], error }) {
    if (!error) {
        return /*#__PURE__*/ _jsxs("html", {
            children: [
                /*#__PURE__*/ _jsx("head", {}),
                /*#__PURE__*/ _jsx("body", {})
            ]
        });
    }
    return /*#__PURE__*/ _jsxs(ErrorBoundary, {
        errorComponent: DefaultGlobalError,
        children: [
            globalErrorStyles,
            /*#__PURE__*/ _jsx(GlobalError, {
                error: error
            })
        ]
    });
}
export class AppDevOverlayErrorBoundary extends PureComponent {
    static getDerivedStateFromError(error) {
        RuntimeErrorHandler.hadRuntimeError = true;
        return {
            reactError: error
        };
    }
    componentDidCatch(err) {
        if (process.env.NODE_ENV === 'development' && err.message === SEGMENT_EXPLORER_SIMULATED_ERROR_MESSAGE) {
            return;
        }
        dispatcher.openErrorOverlay();
    }
    render() {
        const { children, globalError } = this.props;
        const { reactError } = this.state;
        const fallback = /*#__PURE__*/ _jsx(ErroredHtml, {
            globalError: globalError,
            error: reactError
        });
        return reactError !== null ? fallback : children;
    }
    constructor(...args){
        super(...args), this.state = {
            reactError: null
        };
    }
}

//# sourceMappingURL=app-dev-overlay-error-boundary.js.map