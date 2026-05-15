'use client';
import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { HandleISRError } from '../handle-isr-error';
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
    text: {
        fontSize: '14px',
        fontWeight: 400,
        lineHeight: '28px',
        margin: '0 8px'
    }
};
function DefaultGlobalError({ error }) {
    const digest = error?.digest;
    return /*#__PURE__*/ _jsxs("html", {
        id: "__next_error__",
        children: [
            /*#__PURE__*/ _jsx("head", {}),
            /*#__PURE__*/ _jsxs("body", {
                children: [
                    /*#__PURE__*/ _jsx(HandleISRError, {
                        error: error
                    }),
                    /*#__PURE__*/ _jsx("div", {
                        style: styles.error,
                        children: /*#__PURE__*/ _jsxs("div", {
                            children: [
                                /*#__PURE__*/ _jsxs("h2", {
                                    style: styles.text,
                                    children: [
                                        "Application error: a ",
                                        digest ? 'server' : 'client',
                                        "-side exception has occurred while loading ",
                                        window.location.hostname,
                                        " (see the",
                                        ' ',
                                        digest ? 'server logs' : 'browser console',
                                        " for more information)."
                                    ]
                                }),
                                digest ? /*#__PURE__*/ _jsx("p", {
                                    style: styles.text,
                                    children: `Digest: ${digest}`
                                }) : null
                            ]
                        })
                    })
                ]
            })
        ]
    });
}
// Exported so that the import signature in the loaders can be identical to user
// supplied custom global error signatures.
export default DefaultGlobalError;

//# sourceMappingURL=global-error.js.map