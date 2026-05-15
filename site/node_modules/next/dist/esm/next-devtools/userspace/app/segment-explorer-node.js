'use client';
import { jsx as _jsx, jsxs as _jsxs, Fragment as _Fragment } from "react/jsx-runtime";
import { useState, createContext, useContext, use, useMemo, useCallback } from 'react';
import { useLayoutEffect } from 'react';
import { dispatcher } from 'next/dist/compiled/next-devtools';
import { notFound } from '../../../client/components/not-found';
export const SEGMENT_EXPLORER_SIMULATED_ERROR_MESSAGE = 'NEXT_DEVTOOLS_SIMULATED_ERROR';
function SegmentTrieNode({ type, pagePath }) {
    const { boundaryType, setBoundaryType } = useSegmentState();
    const nodeState = useMemo(()=>{
        return {
            type,
            pagePath,
            boundaryType,
            setBoundaryType
        };
    }, [
        type,
        pagePath,
        boundaryType,
        setBoundaryType
    ]);
    // Use `useLayoutEffect` to ensure the state is updated during suspense.
    // `useEffect` won't work as the state is preserved during suspense.
    useLayoutEffect(()=>{
        dispatcher.segmentExplorerNodeAdd(nodeState);
        return ()=>{
            dispatcher.segmentExplorerNodeRemove(nodeState);
        };
    }, [
        nodeState
    ]);
    return null;
}
function NotFoundSegmentNode() {
    notFound();
}
function ErrorSegmentNode() {
    throw Object.defineProperty(new Error(SEGMENT_EXPLORER_SIMULATED_ERROR_MESSAGE), "__NEXT_ERROR_CODE", {
        value: "E394",
        enumerable: false,
        configurable: true
    });
}
const forever = new Promise(()=>{});
function LoadingSegmentNode() {
    use(forever);
    return null;
}
export function SegmentViewStateNode({ page }) {
    useLayoutEffect(()=>{
        dispatcher.segmentExplorerUpdateRouteState(page);
        return ()=>{
            dispatcher.segmentExplorerUpdateRouteState('');
        };
    }, [
        page
    ]);
    return null;
}
export function SegmentBoundaryTriggerNode() {
    const { boundaryType } = useSegmentState();
    let segmentNode = null;
    if (boundaryType === 'loading') {
        segmentNode = /*#__PURE__*/ _jsx(LoadingSegmentNode, {});
    } else if (boundaryType === 'not-found') {
        segmentNode = /*#__PURE__*/ _jsx(NotFoundSegmentNode, {});
    } else if (boundaryType === 'error') {
        segmentNode = /*#__PURE__*/ _jsx(ErrorSegmentNode, {});
    }
    return segmentNode;
}
export function SegmentViewNode({ type, pagePath, children }) {
    const segmentNode = /*#__PURE__*/ _jsx(SegmentTrieNode, {
        type: type,
        pagePath: pagePath
    }, type);
    return /*#__PURE__*/ _jsxs(_Fragment, {
        children: [
            segmentNode,
            children
        ]
    });
}
const SegmentStateContext = /*#__PURE__*/ createContext({
    boundaryType: null,
    setBoundaryType: ()=>{}
});
export function SegmentStateProvider({ children }) {
    const [boundaryType, setBoundaryType] = useState(null);
    const [errorBoundaryKey, setErrorBoundaryKey] = useState(0);
    const reloadBoundary = useCallback(()=>setErrorBoundaryKey((prev)=>prev + 1), []);
    const setBoundaryTypeAndReload = useCallback((type)=>{
        if (type === null) {
            reloadBoundary();
        }
        setBoundaryType(type);
    }, [
        reloadBoundary
    ]);
    return /*#__PURE__*/ _jsx(SegmentStateContext.Provider, {
        value: {
            boundaryType,
            setBoundaryType: setBoundaryTypeAndReload
        },
        children: children
    }, errorBoundaryKey);
}
export function useSegmentState() {
    return useContext(SegmentStateContext);
}

//# sourceMappingURL=segment-explorer-node.js.map