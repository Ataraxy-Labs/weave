"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "default", {
    enumerable: true,
    get: function() {
        return _default;
    }
});
const _interop_require_default = require("@swc/helpers/_/_interop_require_default");
const _jsxruntime = require("react/jsx-runtime");
const _react = /*#__PURE__*/ _interop_require_default._(require("react"));
const styles = {
    error: {
        // https://github.com/sindresorhus/modern-normalize/blob/main/modern-normalize.css#L38-L52
        fontFamily: 'system-ui,"Segoe UI",Roboto,Helvetica,Arial,sans-serif,"Apple Color Emoji","Segoe UI Emoji"',
        height: '100vh',
        textAlign: 'center',
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center'
    },
    desc: {
        lineHeight: '48px'
    },
    h1: {
        display: 'inline-block',
        margin: '0 20px 0 0',
        paddingRight: 23,
        fontSize: 24,
        fontWeight: 500,
        verticalAlign: 'top'
    },
    h2: {
        fontSize: 14,
        fontWeight: 400,
        lineHeight: '28px'
    },
    wrap: {
        display: 'inline-block'
    }
};
/* CSS minified from
body { margin: 0; color: #000; background: #fff; }
.next-error-h1 {
  border-right: 1px solid rgba(0, 0, 0, .3);
}
@media (prefers-color-scheme: dark) {
  body { color: #fff; background: #000; }
  .next-error-h1 {
    border-right: 1px solid rgba(255, 255, 255, .3);
  }
}
*/ const themeCss = `body{color:#000;background:#fff;margin:0}.next-error-h1{border-right:1px solid rgba(0,0,0,.3)}
@media (prefers-color-scheme:dark){body{color:#fff;background:#000}.next-error-h1{border-right:1px solid rgba(255,255,255,.3)}}`;
function AppError() {
    const errorMessage = 'Internal Server Error.';
    const title = `500: ${errorMessage}`;
    return /*#__PURE__*/ (0, _jsxruntime.jsxs)("html", {
        id: "__next_error__",
        children: [
            /*#__PURE__*/ (0, _jsxruntime.jsx)("head", {
                children: /*#__PURE__*/ (0, _jsxruntime.jsx)("title", {
                    children: title
                })
            }),
            /*#__PURE__*/ (0, _jsxruntime.jsx)("body", {
                children: /*#__PURE__*/ (0, _jsxruntime.jsx)("div", {
                    style: styles.error,
                    children: /*#__PURE__*/ (0, _jsxruntime.jsxs)("div", {
                        style: styles.desc,
                        children: [
                            /*#__PURE__*/ (0, _jsxruntime.jsx)("style", {
                                dangerouslySetInnerHTML: {
                                    __html: themeCss
                                }
                            }),
                            /*#__PURE__*/ (0, _jsxruntime.jsx)("h1", {
                                className: "next-error-h1",
                                style: styles.h1,
                                children: "500"
                            }),
                            /*#__PURE__*/ (0, _jsxruntime.jsx)("div", {
                                style: styles.wrap,
                                children: /*#__PURE__*/ (0, _jsxruntime.jsx)("h2", {
                                    style: styles.h2,
                                    children: errorMessage
                                })
                            })
                        ]
                    })
                })
            })
        ]
    });
}
const _default = AppError;

if ((typeof exports.default === 'function' || (typeof exports.default === 'object' && exports.default !== null)) && typeof exports.default.__esModule === 'undefined') {
  Object.defineProperty(exports.default, '__esModule', { value: true });
  Object.assign(exports.default, exports);
  module.exports = exports.default;
}

//# sourceMappingURL=app-error.js.map