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
const _handlers = require("../../server/use-cache/handlers");
const _VAR_MODULE_DOCUMENT = /*#__PURE__*/ _interop_require_default(require("VAR_MODULE_DOCUMENT"));
const _VAR_MODULE_APP = /*#__PURE__*/ _interop_require_wildcard(require("VAR_MODULE_APP"));
const _VAR_USERLAND = /*#__PURE__*/ _interop_require_wildcard(require("VAR_USERLAND"));
const _VAR_MODULE_GLOBAL_ERROR = /*#__PURE__*/ _interop_require_wildcard(require("VAR_MODULE_GLOBAL_ERROR"));
const _module = /*#__PURE__*/ _interop_require_default(require("../../server/route-modules/pages/module"));
const _web = require("../../server/base-http/web");
const _tracer = require("../../server/lib/trace/tracer");
const _constants = require("../../server/lib/trace/constants");
const _constants1 = require("../../lib/constants");
function _interop_require_default(obj) {
    return obj && obj.__esModule ? obj : {
        default: obj
    };
}
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
// INJECT:pageRouteModuleOptions
// INJECT:errorRouteModuleOptions
// INJECT:user500RouteModuleOptions
const pageMod = {
    ..._VAR_USERLAND,
    routeModule: new _module.default({
        ...pageRouteModuleOptions,
        components: {
            App: _VAR_MODULE_APP.default,
            Document: _VAR_MODULE_DOCUMENT.default
        },
        userland: _VAR_USERLAND,
        distDir: process.env.__NEXT_RELATIVE_DIST_DIR || '',
        relativeProjectDir: process.env.__NEXT_RELATIVE_PROJECT_DIR || ''
    })
};
const errorMod = {
    ..._VAR_MODULE_GLOBAL_ERROR,
    routeModule: new _module.default({
        ...errorRouteModuleOptions,
        components: {
            App: _VAR_MODULE_APP.default,
            Document: _VAR_MODULE_DOCUMENT.default
        },
        userland: _VAR_MODULE_GLOBAL_ERROR,
        distDir: process.env.__NEXT_RELATIVE_DIST_DIR || '',
        relativeProjectDir: process.env.__NEXT_RELATIVE_PROJECT_DIR || ''
    })
};
// FIXME: this needs to be made compatible with the template
const error500Mod = userland500Page ? {
    ...userland500Page,
    routeModule: new _module.default({
        ...user500RouteModuleOptions,
        components: {
            App: _VAR_MODULE_APP.default,
            Document: _VAR_MODULE_DOCUMENT.default
        },
        userland: userland500Page,
        distDir: process.env.__NEXT_RELATIVE_DIST_DIR || '',
        relativeProjectDir: process.env.__NEXT_RELATIVE_PROJECT_DIR || ''
    })
} : null;
const ComponentMod = pageMod;
async function requestHandler(req, _event) {
    var _nextConfig_i18n;
    let srcPage = 'VAR_DEFINITION_PATHNAME';
    const relativeUrl = `${req.nextUrl.pathname}${req.nextUrl.search}`;
    const baseReq = new _web.WebNextRequest(req);
    const pageRouteModule = pageMod.routeModule;
    const prepareResult = await pageRouteModule.prepare(baseReq, null, {
        srcPage,
        multiZoneDraftMode: false
    });
    if (!prepareResult) {
        return new Response('Bad Request', {
            status: 400
        });
    }
    const { query, params, buildId, nextConfig, isNextDataRequest, buildManifest, prerenderManifest, reactLoadableManifest, clientReferenceManifest, subresourceIntegrityManifest, dynamicCssManifest } = prepareResult;
    (0, _handlers.initializeCacheHandlers)(nextConfig.cacheMaxMemorySize);
    const renderContext = {
        page: srcPage,
        query,
        params,
        sharedContext: {
            buildId,
            deploymentId: process.env.NEXT_DEPLOYMENT_ID,
            customServer: undefined
        },
        renderContext: {
            isFallback: false,
            isDraftMode: false,
            developmentNotFoundSourcePage: undefined
        },
        renderOpts: {
            params,
            page: srcPage,
            supportsDynamicResponse: true,
            Component: pageMod.Component,
            ComponentMod: pageMod,
            pageConfig: pageMod.pageConfig,
            routeModule: pageMod.routeModule,
            previewProps: prerenderManifest.preview,
            basePath: nextConfig.basePath,
            assetPrefix: nextConfig.assetPrefix,
            images: nextConfig.images,
            optimizeCss: nextConfig.experimental.optimizeCss,
            nextConfigOutput: nextConfig.output,
            nextScriptWorkers: nextConfig.experimental.nextScriptWorkers,
            disableOptimizedLoading: nextConfig.experimental.disableOptimizedLoading,
            domainLocales: (_nextConfig_i18n = nextConfig.i18n) == null ? void 0 : _nextConfig_i18n.domains,
            distDir: '',
            crossOrigin: nextConfig.crossOrigin ? nextConfig.crossOrigin : undefined,
            largePageDataBytes: nextConfig.experimental.largePageDataBytes,
            isExperimentalCompile: nextConfig.experimental.isExperimentalCompile,
            // `htmlLimitedBots` is passed to server as serialized config in string format
            experimental: {
                clientTraceMetadata: nextConfig.experimental.clientTraceMetadata
            },
            buildManifest,
            subresourceIntegrityManifest,
            reactLoadableManifest,
            clientReferenceManifest,
            dynamicCssManifest
        }
    };
    let finalStatus = 200;
    const renderResultToResponse = (result)=>{
        // Handle null responses
        if (result.isNull) {
            finalStatus = 500;
            return new Response(null, {
                status: 500
            });
        }
        // Extract metadata
        const { metadata } = result;
        finalStatus = metadata.statusCode || 200;
        const headers = new Headers();
        // Set content type
        const contentType = result.contentType || _constants1.HTML_CONTENT_TYPE_HEADER;
        headers.set('Content-Type', contentType);
        // Add metadata headers
        if (metadata.headers) {
            for (const [key, value] of Object.entries(metadata.headers)){
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
        }
        // Handle static response
        if (!result.isDynamic) {
            const body = result.toUnchunkedString();
            headers.set('Content-Length', String(new TextEncoder().encode(body).length));
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
        });
        return new Response(readable, {
            status: finalStatus,
            headers
        });
    };
    const invokeRender = async (span)=>{
        try {
            const result = await pageRouteModule.render(// @ts-expect-error we don't type this for edge
            baseReq, new _web.WebNextResponse(undefined), {
                ...renderContext,
                renderOpts: {
                    ...renderContext.renderOpts,
                    getServerSideProps: pageMod.getServerSideProps,
                    Component: pageMod.default || pageMod,
                    ComponentMod: pageMod,
                    pageConfig: pageMod.config,
                    isNextDataRequest
                }
            }).finally(()=>{
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
                const route = rootSpanAttributes.get('next.route');
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
            const errModule = error500Mod || errorMod;
            const errRouteModule = errModule.routeModule;
            if (errRouteModule.isDev) {
                throw err;
            }
            const silenceLog = false;
            await errRouteModule.onRequestError(baseReq, err, {
                routerKind: 'Pages Router',
                routePath: srcPage,
                routeType: 'render',
                revalidateReason: undefined
            }, silenceLog);
            const errResult = await errRouteModule.render(// @ts-expect-error we don't type this for edge
            baseReq, new _web.WebNextResponse(undefined), {
                ...renderContext,
                page: error500Mod ? '/500' : '/_error',
                renderOpts: {
                    ...renderContext.renderOpts,
                    getServerSideProps: errModule.getServerSideProps,
                    Component: errModule.default || errModule,
                    ComponentMod: errModule,
                    pageConfig: errModule.config
                }
            });
            return renderResultToResponse(errResult);
        }
    };
    const tracer = (0, _tracer.getTracer)();
    return tracer.withPropagatedContext(req.headers, ()=>tracer.trace(_constants.BaseServerSpan.handleRequest, {
            spanName: `${req.method} ${srcPage}`,
            kind: _tracer.SpanKind.SERVER,
            attributes: {
                'http.method': req.method,
                'http.target': relativeUrl,
                'http.route': srcPage
            }
        }, invokeRender));
}
const handler = (opts)=>{
    return (0, _adapter.adapter)({
        ...opts,
        IncrementalCache: _incrementalcache.IncrementalCache,
        handler: requestHandler,
        incrementalCacheHandler,
        bypassNextUrl: true,
        page: 'VAR_DEFINITION_PATHNAME'
    });
};
const _default = handler;

//# sourceMappingURL=edge-ssr.js.map