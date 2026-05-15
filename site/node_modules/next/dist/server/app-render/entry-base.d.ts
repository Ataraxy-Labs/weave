export { createTemporaryReferenceSet, renderToReadableStream, decodeReply, decodeAction, decodeFormState, } from 'react-server-dom-webpack/server';
export { prerender } from 'react-server-dom-webpack/static';
export { captureOwnerStack, createElement, Fragment } from 'react';
export { default as LayoutRouter } from '../../client/components/layout-router';
export { default as RenderFromTemplateContext } from '../../client/components/render-from-template-context';
export { workAsyncStorage } from '../app-render/work-async-storage.external';
export { workUnitAsyncStorage } from './work-unit-async-storage.external';
export { actionAsyncStorage } from '../app-render/action-async-storage.external';
export { ClientPageRoot } from '../../client/components/client-page';
export { ClientSegmentRoot } from '../../client/components/client-segment';
export { createServerSearchParamsForServerPage, createPrerenderSearchParamsForClientPage, } from '../request/search-params';
export { createServerParamsForServerSegment, createPrerenderParamsForClientSegment, } from '../request/params';
export * as serverHooks from '../../client/components/hooks-server-context';
export { HTTPAccessFallbackBoundary } from '../../client/components/http-access-fallback/error-boundary';
export { createMetadataComponents } from '../../lib/metadata/metadata';
export { RootLayoutBoundary } from '../../lib/framework/boundary-components';
export { preloadStyle, preloadFont, preconnect } from './rsc/preloads';
export { Postpone } from './rsc/postpone';
export { taintObjectReference } from './rsc/taint';
export { collectSegmentData } from './collect-segment-data';
declare let SegmentViewNode: typeof import('../../next-devtools/userspace/app/segment-explorer-node').SegmentViewNode;
declare let SegmentViewStateNode: typeof import('../../next-devtools/userspace/app/segment-explorer-node').SegmentViewStateNode;
declare global {
    var __next__clear_chunk_cache__: (() => void) | null | undefined;
    var __turbopack_clear_chunk_cache__: () => void | null | undefined;
}
export declare function patchFetch(): void;
export { SegmentViewNode, SegmentViewStateNode };
