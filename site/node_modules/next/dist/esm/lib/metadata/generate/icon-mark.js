'use client';
import { jsx as _jsx } from "react/jsx-runtime";
// This is a client component that only renders during SSR,
// but will be replaced during streaming with an icon insertion script tag.
// We don't want it to be presented anywhere so it's only visible during streaming,
// right after the icon meta tags so that browser can pick it up as soon as it's rendered.
// Note: we don't just emit the script here because we only need the script if it's not in the head,
// and we need it to be hoistable alongside the other metadata but sync scripts are not hoistable.
export const IconMark = ()=>{
    if (typeof window !== 'undefined') {
        return null;
    }
    return /*#__PURE__*/ _jsx("meta", {
        name: "\xabnxt-icon\xbb"
    });
};

//# sourceMappingURL=icon-mark.js.map