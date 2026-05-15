import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import React from 'react';
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
    return /*#__PURE__*/ _jsxs("html", {
        id: "__next_error__",
        children: [
            /*#__PURE__*/ _jsx("head", {
                children: /*#__PURE__*/ _jsx("title", {
                    children: title
                })
            }),
            /*#__PURE__*/ _jsx("body", {
                children: /*#__PURE__*/ _jsx("div", {
                    style: styles.error,
                    children: /*#__PURE__*/ _jsxs("div", {
                        style: styles.desc,
                        children: [
                            /*#__PURE__*/ _jsx("style", {
                                dangerouslySetInnerHTML: {
                                    __html: themeCss
                                }
                            }),
                            /*#__PURE__*/ _jsx("h1", {
                                className: "next-error-h1",
                                style: styles.h1,
                                children: "500"
                            }),
                            /*#__PURE__*/ _jsx("div", {
                                style: styles.wrap,
                                children: /*#__PURE__*/ _jsx("h2", {
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
export default AppError;

//# sourceMappingURL=app-error.js.map