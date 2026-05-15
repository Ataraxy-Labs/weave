import { jsx as _jsx } from "react/jsx-runtime";
import { HTTPAccessErrorFallback } from '../http-access-fallback/error-fallback';
function GlobalNotFound() {
    return /*#__PURE__*/ _jsx("html", {
        children: /*#__PURE__*/ _jsx("body", {
            children: /*#__PURE__*/ _jsx(HTTPAccessErrorFallback, {
                status: 404,
                message: 'This page could not be found.'
            })
        })
    });
}
export default GlobalNotFound;

//# sourceMappingURL=global-not-found.js.map