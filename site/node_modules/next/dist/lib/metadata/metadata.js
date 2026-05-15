"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "createMetadataComponents", {
    enumerable: true,
    get: function() {
        return createMetadataComponents;
    }
});
const _jsxruntime = require("react/jsx-runtime");
const _react = /*#__PURE__*/ _interop_require_wildcard(require("react"));
const _basic = require("./generate/basic");
const _alternate = require("./generate/alternate");
const _opengraph = require("./generate/opengraph");
const _icons = require("./generate/icons");
const _resolvemetadata = require("./resolve-metadata");
const _meta = require("./generate/meta");
const _httpaccessfallback = require("../../client/components/http-access-fallback/http-access-fallback");
const _searchparams = require("../../server/request/search-params");
const _pathname = require("../../server/request/pathname");
const _ispostpone = require("../../server/lib/router-utils/is-postpone");
const _boundarycomponents = require("../framework/boundary-components");
function _getRequireWildcardCache(nodeInterop) {
    if (typeof WeakMap !== "function") return null;
    var cacheBabelInterop = new WeakMap();
    var cacheNodeInterop = new WeakMap();
    return (_getRequireWildcardCache = function(nodeInterop) {
        return nodeInterop ? cacheNodeInterop : cacheBabelInterop;
    })(nodeInterop);
}
function _interop_require_wildcard(obj, nodeInterop) {
    if (!nodeInterop && obj && obj.__esModule) {
        return obj;
    }
    if (obj === null || typeof obj !== "object" && typeof obj !== "function") {
        return {
            default: obj
        };
    }
    var cache = _getRequireWildcardCache(nodeInterop);
    if (cache && cache.has(obj)) {
        return cache.get(obj);
    }
    var newObj = {
        __proto__: null
    };
    var hasPropertyDescriptor = Object.defineProperty && Object.getOwnPropertyDescriptor;
    for(var key in obj){
        if (key !== "default" && Object.prototype.hasOwnProperty.call(obj, key)) {
            var desc = hasPropertyDescriptor ? Object.getOwnPropertyDescriptor(obj, key) : null;
            if (desc && (desc.get || desc.set)) {
                Object.defineProperty(newObj, key, desc);
            } else {
                newObj[key] = obj[key];
            }
        }
    }
    newObj.default = obj;
    if (cache) {
        cache.set(obj, newObj);
    }
    return newObj;
}
function createMetadataComponents({ tree, pathname, parsedQuery, metadataContext, getDynamicParamFromSegment, errorType, workStore, serveStreamingMetadata }) {
    const searchParams = (0, _searchparams.createServerSearchParamsForMetadata)(parsedQuery, workStore);
    const pathnameForMetadata = (0, _pathname.createServerPathnameForMetadata)(pathname, workStore);
    async function Viewport() {
        const tags = await getResolvedViewport(tree, searchParams, getDynamicParamFromSegment, workStore, errorType).catch((viewportErr)=>{
            // When Legacy PPR is enabled viewport can reject with a Postpone type
            // This will go away once Legacy PPR is removed and dynamic metadata will
            // stay pending until after the prerender is complete when it is dynamic
            if ((0, _ispostpone.isPostpone)(viewportErr)) {
                throw viewportErr;
            }
            if (!errorType && (0, _httpaccessfallback.isHTTPAccessFallbackError)(viewportErr)) {
                return getNotFoundViewport(tree, searchParams, getDynamicParamFromSegment, workStore).catch(()=>null);
            }
            // We're going to throw the error from the metadata outlet so we just render null here instead
            return null;
        });
        return tags;
    }
    Viewport.displayName = 'Next.Viewport';
    function ViewportWrapper() {
        return /*#__PURE__*/ (0, _jsxruntime.jsx)(_boundarycomponents.ViewportBoundary, {
            children: /*#__PURE__*/ (0, _jsxruntime.jsx)(Viewport, {})
        });
    }
    async function Metadata() {
        const tags = await getResolvedMetadata(tree, pathnameForMetadata, searchParams, getDynamicParamFromSegment, metadataContext, workStore, errorType).catch((metadataErr)=>{
            // When Legacy PPR is enabled metadata can reject with a Postpone type
            // This will go away once Legacy PPR is removed and dynamic metadata will
            // stay pending until after the prerender is complete when it is dynamic
            if ((0, _ispostpone.isPostpone)(metadataErr)) {
                throw metadataErr;
            }
            if (!errorType && (0, _httpaccessfallback.isHTTPAccessFallbackError)(metadataErr)) {
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
            return /*#__PURE__*/ (0, _jsxruntime.jsx)(_boundarycomponents.MetadataBoundary, {
                children: /*#__PURE__*/ (0, _jsxruntime.jsx)(Metadata, {})
            });
        }
        return /*#__PURE__*/ (0, _jsxruntime.jsx)("div", {
            hidden: true,
            children: /*#__PURE__*/ (0, _jsxruntime.jsx)(_boundarycomponents.MetadataBoundary, {
                children: /*#__PURE__*/ (0, _jsxruntime.jsx)(_react.Suspense, {
                    name: "Next.Metadata",
                    children: /*#__PURE__*/ (0, _jsxruntime.jsx)(Metadata, {})
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
            return /*#__PURE__*/ (0, _jsxruntime.jsx)(_boundarycomponents.OutletBoundary, {
                children: pendingOutlet
            });
        }
        return /*#__PURE__*/ (0, _jsxruntime.jsx)(_boundarycomponents.OutletBoundary, {
            children: /*#__PURE__*/ (0, _jsxruntime.jsx)(_react.Suspense, {
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
const getResolvedMetadata = (0, _react.cache)(getResolvedMetadataImpl);
async function getResolvedMetadataImpl(tree, pathname, searchParams, getDynamicParamFromSegment, metadataContext, workStore, errorType) {
    const errorConvention = errorType === 'redirect' ? undefined : errorType;
    return renderMetadata(tree, pathname, searchParams, getDynamicParamFromSegment, metadataContext, workStore, errorConvention);
}
const getNotFoundMetadata = (0, _react.cache)(getNotFoundMetadataImpl);
async function getNotFoundMetadataImpl(tree, pathname, searchParams, getDynamicParamFromSegment, metadataContext, workStore) {
    const notFoundErrorConvention = 'not-found';
    return renderMetadata(tree, pathname, searchParams, getDynamicParamFromSegment, metadataContext, workStore, notFoundErrorConvention);
}
const getResolvedViewport = (0, _react.cache)(getResolvedViewportImpl);
async function getResolvedViewportImpl(tree, searchParams, getDynamicParamFromSegment, workStore, errorType) {
    const errorConvention = errorType === 'redirect' ? undefined : errorType;
    return renderViewport(tree, searchParams, getDynamicParamFromSegment, workStore, errorConvention);
}
const getNotFoundViewport = (0, _react.cache)(getNotFoundViewportImpl);
async function getNotFoundViewportImpl(tree, searchParams, getDynamicParamFromSegment, workStore) {
    const notFoundErrorConvention = 'not-found';
    return renderViewport(tree, searchParams, getDynamicParamFromSegment, workStore, notFoundErrorConvention);
}
async function renderMetadata(tree, pathname, searchParams, getDynamicParamFromSegment, metadataContext, workStore, errorConvention) {
    const resolvedMetadata = await (0, _resolvemetadata.resolveMetadata)(tree, pathname, searchParams, errorConvention, getDynamicParamFromSegment, workStore, metadataContext);
    const elements = createMetadataElements(resolvedMetadata);
    return /*#__PURE__*/ (0, _jsxruntime.jsx)(_jsxruntime.Fragment, {
        children: elements.map((el, index)=>{
            return /*#__PURE__*/ (0, _react.cloneElement)(el, {
                key: index
            });
        })
    });
}
async function renderViewport(tree, searchParams, getDynamicParamFromSegment, workStore, errorConvention) {
    const resolvedViewport = await (0, _resolvemetadata.resolveViewport)(tree, searchParams, errorConvention, getDynamicParamFromSegment, workStore);
    const elements = createViewportElements(resolvedViewport);
    return /*#__PURE__*/ (0, _jsxruntime.jsx)(_jsxruntime.Fragment, {
        children: elements.map((el, index)=>{
            return /*#__PURE__*/ (0, _react.cloneElement)(el, {
                key: index
            });
        })
    });
}
function createMetadataElements(metadata) {
    return (0, _meta.MetaFilter)([
        (0, _basic.BasicMeta)({
            metadata
        }),
        (0, _alternate.AlternatesMetadata)({
            alternates: metadata.alternates
        }),
        (0, _basic.ItunesMeta)({
            itunes: metadata.itunes
        }),
        (0, _basic.FacebookMeta)({
            facebook: metadata.facebook
        }),
        (0, _basic.PinterestMeta)({
            pinterest: metadata.pinterest
        }),
        (0, _basic.FormatDetectionMeta)({
            formatDetection: metadata.formatDetection
        }),
        (0, _basic.VerificationMeta)({
            verification: metadata.verification
        }),
        (0, _basic.AppleWebAppMeta)({
            appleWebApp: metadata.appleWebApp
        }),
        (0, _opengraph.OpenGraphMetadata)({
            openGraph: metadata.openGraph
        }),
        (0, _opengraph.TwitterMetadata)({
            twitter: metadata.twitter
        }),
        (0, _opengraph.AppLinksMeta)({
            appLinks: metadata.appLinks
        }),
        (0, _icons.IconsMetadata)({
            icons: metadata.icons
        })
    ]);
}
function createViewportElements(viewport) {
    return (0, _meta.MetaFilter)([
        (0, _basic.ViewportMeta)({
            viewport: viewport
        })
    ]);
}

//# sourceMappingURL=metadata.js.map