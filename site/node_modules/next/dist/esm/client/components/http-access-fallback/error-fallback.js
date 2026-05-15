import { jsx as _jsx, jsxs as _jsxs, Fragment as _Fragment } from "react/jsx-runtime";
import { styles } from '../styles/access-error-styles';
export function HTTPAccessErrorFallback({ status, message }) {
    return /*#__PURE__*/ _jsxs(_Fragment, {
        children: [
            /*#__PURE__*/ _jsx("title", {
                children: `${status}: ${message}`
            }),
            /*#__PURE__*/ _jsx("div", {
                style: styles.error,
                children: /*#__PURE__*/ _jsxs("div", {
                    children: [
                        /*#__PURE__*/ _jsx("style", {
                            dangerouslySetInnerHTML: {
                                /* Minified CSS from
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
              */ __html: `body{color:#000;background:#fff;margin:0}.next-error-h1{border-right:1px solid rgba(0,0,0,.3)}@media (prefers-color-scheme:dark){body{color:#fff;background:#000}.next-error-h1{border-right:1px solid rgba(255,255,255,.3)}}`
                            }
                        }),
                        /*#__PURE__*/ _jsx("h1", {
                            className: "next-error-h1",
                            style: styles.h1,
                            children: status
                        }),
                        /*#__PURE__*/ _jsx("div", {
                            style: styles.desc,
                            children: /*#__PURE__*/ _jsx("h2", {
                                style: styles.h2,
                                children: message
                            })
                        })
                    ]
                })
            })
        ]
    });
}

//# sourceMappingURL=error-fallback.js.map