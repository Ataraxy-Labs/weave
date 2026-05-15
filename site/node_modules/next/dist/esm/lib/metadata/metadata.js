import { jsx as _jsx, Fragment as _Fragment } from "react/jsx-runtime";
import React, { Suspense, cache, cloneElement } from 'react';
import { AppleWebAppMeta, FormatDetectionMeta, ItunesMeta, BasicMeta, ViewportMeta, VerificationMeta, FacebookMeta, PinterestMeta } from './generate/basic';
import { AlternatesMetadata } from './generate/alternate';
import { OpenGraphMetadata, TwitterMetadata, AppLinksMeta } from './generate/opengraph';
import { IconsMetadata } from './generate/icons';
import { resolveMetadata, resolveViewport } from './resolve-metadata';
import { MetaFilter } from './generate/meta';
import { isHTTPAccessFallbackError } from '../../client/components/http-access-fallback/http-access-fallback';
import { createServerSearchParamsForMetadata } from '../../server/request/search-params';
import { createServerPathnameForMetadata } from '../../server/request/pathname';
import { isPostpone } from '../../server/lib/router-utils/is-postpone';
import { MetadataBoundary, ViewportBoundary, OutletBoundary } from '../framework/boundary-components';
// Use a promise to share the status of the metadata resolving,
// returning two components `MetadataTree` and `MetadataOutlet`
// `MetadataTree` is the one that will be rendered at first in the content sequence for metadata tags.
// `MetadataOutlet` is the one that will be rendered under error boundaries for metadata resolving errors.
// In this way we can let the metadata tags always render successfully,
// and the error will be caught by the error boundary and trigger fallbacks.
export function createMetadataComponents({ tree, pathname, parsedQuery, metadataContext, getDynamicParamFromSegment, errorType, workStore, serveStreamingMetadata }) {
    const searchParams = createServerSearchParamsForMetadata(parsedQuery, workStore);
    const pathnameForMetadata = createServerPathnameForMetadata(pathname, workStore);
    async function Viewport() {
        const tags = await getResolvedViewport(tree, searchParams, getDynamicParamFromSegment, workStore, errorType).catch((viewportErr)=>{
            // When Legacy PPR is enabled viewport can reject with a Postpone type
            // This will go away once Legacy PPR is removed and dynamic metadata will
            // stay pending until after the prerender is complete when it is dynamic
            if (isPostpone(viewportErr)) {
                throw viewportErr;
            }
            if (!errorType && isHTTPAccessFallbackError(viewportErr)) {
                return getNotFoundViewport(tree, searchParams, getDynamicParamFromSegment, workStore).catch(()=>null);
            }
            // We're going to throw the error from the metadata outlet so we just render null here instead
            return null;
        });
        return tags;
    }
    Viewport.displayName = 'Next.Viewport';
    function ViewportWrapper() {
        return /*#__PURE__*/ _jsx(ViewportBoundary, {
            children: /*#__PURE__*/ _jsx(Viewport, {})
        });
    }
    async function Metadata() {
        const tags = await getResolvedMetadata(tree, pathnameForMetadata, searchParams, getDynamicParamFromSegment, metadataContext, workStore, errorType).catch((metadataErr)=>{
            // When Legacy PPR is enabled metadata can reject with a Postpone type
            // This will go away once Legacy PPR is removed and dynamic metadata will
            // stay pending until after the prerender is complete when it is dynamic
            if (isPostpone(metadataErr)) {
                throw metadataErr;
            }
            if (!errorType && isHTTPAccessFallbackError(metadataErr)) {
                return getNotFoundMetadata(tree, pathnameForMetadata, searchParams, getDynamicParamFromSegment, metadataContext, workStore).catch(()=>null);
            }
            // We're going to throw the error from the metadata outlet so we just render null here instead
            return null;
        });
        return tags;
    }
    Metadata.displayName = 'Next.Metadata';
    function MetadataWrapper() {
        // TODO: We shouldn't change what we render based on whether we are streaming or not.
        // If we aren't streaming we should just block the response until we have resolved the
        // metadata.
        if (!serveStreamingMetadata) {
            return /*#__PURE__*/ _jsx(MetadataBoundary, {
                children: /*#__PURE__*/ _jsx(Metadata, {})
            });
        }
        return /*#__PURE__*/ _jsx("div", {
            hidden: true,
            children: /*#__PURE__*/ _jsx(MetadataBoundary, {
                children: /*#__PURE__*/ _jsx(Suspense, {
                    name: "Next.Metadata",
                    children: /*#__PURE__*/ _jsx(Metadata, {})
                })
            })
        });
    }
    function MetadataOutlet() {
        const pendingOutlet = Promise.all([
            getResolvedMetadata(tree, pathnameForMetadata, searchParams, getDynamicParamFromSegment, metadataContext, workStore, errorType),
            getResolvedViewport(tree, searchParams, getDynamicParamFromSegment, workStore, errorType)
        ]).then(()=>null);
        // TODO: We shouldn't change what we render based on whether we are streaming or not.
        // If we aren't streaming we should just block the response until we have resolved the
        // metadata.
        if (!serveStreamingMetadata) {
            return /*#__PURE__*/ _jsx(OutletBoundary, {
                children: pendingOutlet
            });
        }
        return /*#__PURE__*/ _jsx(OutletBoundary, {
            children: /*#__PURE__*/ _jsx(Suspense, {
                name: "Next.MetadataOutlet",
                children: pendingOutlet
            })
        });
    }
    MetadataOutlet.displayName = 'Next.MetadataOutlet';
    return {
        Viewport: ViewportWrapper,
        Metadata: MetadataWrapper,
        MetadataOutlet
    };
}
const getResolvedMetadata = cache(getResolvedMetadataImpl);
async function getResolvedMetadataImpl(tree, pathname, searchParams, getDynamicParamFromSegment, metadataContext, workStore, errorType) {
    const errorConvention = errorType === 'redirect' ? undefined : errorType;
    return renderMetadata(tree, pathname, searchParams, getDynamicParamFromSegment, metadataContext, workStore, errorConvention);
}
const getNotFoundMetadata = cache(getNotFoundMetadataImpl);
async function getNotFoundMetadataImpl(tree, pathname, searchParams, getDynamicParamFromSegment, metadataContext, workStore) {
    const notFoundErrorConvention = 'not-found';
    return renderMetadata(tree, pathname, searchParams, getDynamicParamFromSegment, metadataContext, workStore, notFoundErrorConvention);
}
const getResolvedViewport = cache(getResolvedViewportImpl);
async function getResolvedViewportImpl(tree, searchParams, getDynamicParamFromSegment, workStore, errorType) {
    const errorConvention = errorType === 'redirect' ? undefined : errorType;
    return renderViewport(tree, searchParams, getDynamicParamFromSegment, workStore, errorConvention);
}
const getNotFoundViewport = cache(getNotFoundViewportImpl);
async function getNotFoundViewportImpl(tree, searchParams, getDynamicParamFromSegment, workStore) {
    const notFoundErrorConvention = 'not-found';
    return renderViewport(tree, searchParams, getDynamicParamFromSegment, workStore, notFoundErrorConvention);
}
async function renderMetadata(tree, pathname, searchParams, getDynamicParamFromSegment, metadataContext, workStore, errorConvention) {
    const resolvedMetadata = await resolveMetadata(tree, pathname, searchParams, errorConvention, getDynamicParamFromSegment, workStore, metadataContext);
    const elements = createMetadataElements(resolvedMetadata);
    return /*#__PURE__*/ _jsx(_Fragment, {
        children: elements.map((el, index)=>{
            return /*#__PURE__*/ cloneElement(el, {
                key: index
            });
        })
    });
}
async function renderViewport(tree, searchParams, getDynamicParamFromSegment, workStore, errorConvention) {
    const resolvedViewport = await resolveViewport(tree, searchParams, errorConvention, getDynamicParamFromSegment, workStore);
    const elements = createViewportElements(resolvedViewport);
    return /*#__PURE__*/ _jsx(_Fragment, {
        children: elements.map((el, index)=>{
            return /*#__PURE__*/ cloneElement(el, {
                key: index
            });
        })
    });
}
function createMetadataElements(metadata) {
    return MetaFilter([
        BasicMeta({
            metadata
        }),
        AlternatesMetadata({
            alternates: metadata.alternates
        }),
        ItunesMeta({
            itunes: metadata.itunes
        }),
        FacebookMeta({
            facebook: metadata.facebook
        }),
        PinterestMeta({
            pinterest: metadata.pinterest
        }),
        FormatDetectionMeta({
            formatDetection: metadata.formatDetection
        }),
        VerificationMeta({
            verification: metadata.verification
        }),
        AppleWebAppMeta({
            appleWebApp: metadata.appleWebApp
        }),
        OpenGraphMetadata({
            openGraph: metadata.openGraph
        }),
        TwitterMetadata({
            twitter: metadata.twitter
        }),
        AppLinksMeta({
            appLinks: metadata.appLinks
        }),
        IconsMetadata({
            icons: metadata.icons
        })
    ]);
}
function createViewportElements(viewport) {
    return MetaFilter([
        ViewportMeta({
            viewport: viewport
        })
    ]);
}

//# sourceMappingURL=metadata.js.map