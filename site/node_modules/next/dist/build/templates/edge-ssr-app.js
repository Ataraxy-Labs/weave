"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
0 && (module.exports = {
    ComponentMod: null,
    default: null
});
function _export(target, all) {
    for(var name in all)Object.defineProperty(target, name, {
        enumerable: true,
        get: all[name]
    });
}
_export(exports, {
    ComponentMod: function() {
        return ComponentMod;
    },
    default: function() {
        return _default;
    }
});
require("../../server/web/globals");
const _adapter = require("../../server/web/adapter");
const _incrementalcache = require("../../server/lib/incremental-cache");
const _VAR_USERLAND = /*#__PURE__*/ _interop_require_wildcard(require("VAR_USERLAND"));
const _manifestssingleton = require("../../server/app-render/manifests-singleton");
const _handlers = require("../../server/use-cache/handlers");
const _constants = require("../../server/lib/trace/constants");
const _tracer = require("../../server/lib/trace/tracer");
const _web = require("../../server/base-http/web");
const _serveractionrequestmeta = require("../../server/lib/server-action-request-meta");
const _isbot = require("../../shared/lib/router/utils/is-bot");
const _interopdefault = require("../../lib/interop-default");
const _apppaths = require("../../shared/lib/router/utils/app-paths");
const _apiutils = require("../../server/api-utils");
const _webonclose = require("../../server/web/web-on-close");
const _sizelimit = require("../../shared/lib/size-limit");
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
var _self___RSC_MANIFEST;
// OPTIONAL_IMPORT:incrementalCacheHandler
const maybeJSONParse = (str)=>str ? JSON.parse(str) : undefined;
const rscManifest = (_self___RSC_MANIFEST = self.__RSC_MANIFEST) == null ? void 0 : _self___RSC_MANIFEST['VAR_PAGE'];
const rscServerManifest = maybeJSONParse(self.__RSC_SERVER_MANIFEST);
if (rscManifest && rscServerManifest) {
    (0, _manifestssingleton.setManifestsSingleton)({
        page: 'VAR_PAGE',
        clientReferenceManifest: rscManifest,
        serverActionsManifest: rscServerManifest
    });
}
const ComponentMod = _VAR_USERLAND;
async function requestHandler(req, event) {
    let srcPage = 'VAR_PAGE';
    const normalizedSrcPage = (0, _apppaths.normalizeAppPath)(srcPage);
    const relativeUrl = `${req.nextUrl.pathname}${req.nextUrl.search}`;
    const baseReq = new _web.WebNextRequest(req);
    const baseRes = new _web.WebNextResponse(undefined);
    const pageRouteModule = _VAR_USERLAND.routeModule;
    const prepareResult = await pageRouteModule.prepare(baseReq, null, {
        srcPage,
        multiZoneDraftMode: false
    });
    if (!prepareResult) {
        return new Response('Bad Request', {
            status: 400
        });
    }
    const { query, params, buildId, nextConfig, buildManifest, prerenderManifest, reactLoadableManifest, subresourceIntegrityManifest, dynamicCssManifest, nextFontManifest, resolvedPathname, interceptionRoutePatterns, routerServerContext, deploymentId } = prepareResult;
    // Initialize the cache handlers interface.
    (0, _handlers.initializeCacheHandlers)(nextConfig.cacheMaxMemorySize);
    const isPossibleServerAction = (0, _serveractionrequestmeta.getIsPossibleServerAction)(req);
    const botType = (0, _isbot.getBotType)(req.headers.get('User-Agent') || '');
    const { isOnDemandRevalidate } = (0, _apiutils.checkIsOnDemandRevalidate)(req, prerenderManifest.preview);
    const closeController = new _webonclose.CloseController();
    const renderContext = {
        page: normalizedSrcPage,
        query,
        params,
        sharedContext: {
            buildId
        },
        fallbackRouteParams: null,
        renderOpts: {
            App: ()=>null,
            Document: ()=>null,
            pageConfig: {},
            ComponentMod,
            Component: (0, _interopdefault.interopDefault)(ComponentMod),
            routeModule: pageRouteModule,
            params,
            page: srcPage,
            postponed: undefined,
            shouldWaitOnAllReady: false,
            serveStreamingMetadata: true,
            supportsDynamicResponse: true,
            buildManifest,
            nextFontManifest,
            reactLoadableManifest,
            subresourceIntegrityManifest,
            dynamicCssManifest,
            setIsrStatus: routerServerContext == null ? void 0 : routerServerContext.setIsrStatus,
            dir: pageRouteModule.relativeProjectDir,
            botType,
            isDraftMode: false,
            isOnDemandRevalidate,
            isPossibleServerAction,
            assetPrefix: nextConfig.assetPrefix,
            nextConfigOutput: nextConfig.output,
            crossOrigin: nextConfig.crossOrigin,
            trailingSlash: nextConfig.trailingSlash,
            images: nextConfig.images,
            previewProps: prerenderManifest.preview,
            deploymentId,
            enableTainting: nextConfig.experimental.taint,
            htmlLimitedBots: nextConfig.htmlLimitedBots,
            reactMaxHeadersLength: nextConfig.reactMaxHeadersLength,
            multiZoneDraftMode: false,
            cacheLifeProfiles: nextConfig.cacheLife,
            basePath: nextConfig.basePath,
            serverActions: nextConfig.experimental.serverActions,
            cacheComponents: Boolean(nextConfig.cacheComponents),
            experimental: {
                isRoutePPREnabled: false,
                expireTime: nextConfig.expireTime,
                staleTimes: nextConfig.experimental.staleTimes,
                dynamicOnHover: Boolean(nextConfig.experimental.dynamicOnHover),
                inlineCss: Boolean(nextConfig.experimental.inlineCss),
                authInterrupts: Boolean(nextConfig.experimental.authInterrupts),
                clientTraceMetadata: nextConfig.experimental.clientTraceMetadata || [],
                clientParamParsingOrigins: nextConfig.experimental.clientParamParsingOrigins,
                maxPostponedStateSizeBytes: (0, _sizelimit.parseMaxPostponedStateSize)(nextConfig.experimental.maxPostponedStateSize)
            },
            incrementalCache: await pageRouteModule.getIncrementalCache(baseReq, nextConfig, prerenderManifest, true),
            waitUntil: event.waitUntil.bind(event),
            onClose: (cb)=>{
                closeController.onClose(cb);
            },
            onAfterTaskError: ()=>{},
            onInstrumentationRequestError: (error, _request, errorContext, silenceLog)=>pageRouteModule.onRequestError(baseReq, error, errorContext, silenceLog, routerServerContext),
            dev: pageRouteModule.isDev
        }
    };
    let finalStatus = 200;
    const renderResultToResponse = (result)=>{
        const varyHeader = pageRouteModule.getVaryHeader(resolvedPathname, interceptionRoutePatterns);
        // Handle null responses
        if (result.isNull) {
            finalStatus = 500;
            closeController.dispatchClose();
            return new Response(null, {
                status: 500
            });
        }
        // Extract metadata
        const { metadata } = result;
        const headers = new Headers();
        finalStatus = metadata.statusCode || baseRes.statusCode || 200;
        req.fetchMetrics = metadata.fetchMetrics;
        // Set content type
        const contentType = result.contentType || 'text/html; charset=utf-8';
        headers.set('Content-Type', contentType);
        headers.set('x-edge-runtime', '1');
        if (varyHeader) {
            headers.set('Vary', varyHeader);
        }
        // Add existing headers
        for (const [key, value] of Object.entries({
            ...baseRes.getHeaders(),
            ...metadata.headers
        })){
            if (value !== undefined) {
                if (Array.isArray(value)) {
                    // Handle multiple header values
                    for (const v of value){
                        headers.append(key, String(v));
                    }
                } else {
                    headers.set(key, String(value));
                }
            }
        }
        // Handle static response
        if (!result.isDynamic) {
            const body = result.toUnchunkedString();
            headers.set('Content-Length', String(new TextEncoder().encode(body).length));
            closeController.dispatchClose();
            return new Response(body, {
                status: finalStatus,
                headers
            });
        }
        // Handle dynamic/streaming response
        // For edge runtime, we need to create a readable stream that pipes from the result
        const { readable, writable } = new TransformStream();
        // Start piping the result to the writable stream
        // This is done asynchronously to avoid blocking the response creation
        result.pipeTo(writable).catch((err)=>{
            console.error('Error piping RenderResult to response:', err);
        }).finally(()=>closeController.dispatchClose());
        return new Response(readable, {
            status: finalStatus,
            headers
        });
    };
    const invokeRender = async (span)=>{
        try {
            const result = await pageRouteModule.render(baseReq, baseRes, renderContext).finally(()=>{
                if (!span) return;
                span.setAttributes({
                    'http.status_code': finalStatus,
                    'next.rsc': false
                });
                const rootSpanAttributes = tracer.getRootSpanAttributes();
                // We were unable to get attributes, probably OTEL is not enabled
                if (!rootSpanAttributes) {
                    return;
                }
                if (rootSpanAttributes.get('next.span_type') !== _constants.BaseServerSpan.handleRequest) {
                    console.warn(`Unexpected root span type '${rootSpanAttributes.get('next.span_type')}'. Please report this Next.js issue https://github.com/vercel/next.js`);
                    return;
                }
                const route = normalizedSrcPage;
                if (route) {
                    const name = `${req.method} ${route}`;
                    span.setAttributes({
                        'next.route': route,
                        'http.route': route,
                        'next.span_name': name
                    });
                    span.updateName(name);
                } else {
                    span.updateName(`${req.method} ${srcPage}`);
                }
            });
            return renderResultToResponse(result);
        } catch (err) {
            const silenceLog = false;
            await pageRouteModule.onRequestError(baseReq, err, {
                routerKind: 'App Router',
                routePath: normalizedSrcPage,
                routeType: 'render',
                revalidateReason: undefined
            }, silenceLog);
            // rethrow so that we can handle serving error page
            throw err;
        }
    };
    const tracer = (0, _tracer.getTracer)();
    return tracer.withPropagatedContext(req.headers, ()=>tracer.trace(_constants.BaseServerSpan.handleRequest, {
            spanName: `${req.method} ${srcPage}`,
            kind: _tracer.SpanKind.SERVER,
            attributes: {
                'http.method': req.method,
                'http.target': relativeUrl,
                'http.route': normalizedSrcPage
            }
        }, invokeRender));
}
const handler = (opts)=>{
    return (0, _adapter.adapter)({
        ...opts,
        IncrementalCache: _incrementalcache.IncrementalCache,
        handler: requestHandler,
        incrementalCacheHandler,
        page: 'VAR_PAGE'
    });
};
const _default = handler;

//# sourceMappingURL=edge-ssr-app.js.map