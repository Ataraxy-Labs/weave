import { renderToHTMLOrFlight } from '../../app-render/app-render';
import { RouteModule } from '../route-module';
import * as vendoredContexts from './vendored/contexts/entrypoints';
import { PrerenderManifestMatcher } from './helpers/prerender-manifest-matcher';
import { NEXT_ROUTER_PREFETCH_HEADER, NEXT_ROUTER_SEGMENT_PREFETCH_HEADER, NEXT_ROUTER_STATE_TREE_HEADER, NEXT_URL, RSC_HEADER } from '../../../client/components/app-router-headers';
import { isInterceptionRouteAppPath } from '../../../shared/lib/router/utils/interception-routes';
let vendoredReactRSC;
let vendoredReactSSR;
// the vendored Reacts are loaded from their original source in the edge runtime
if (process.env.NEXT_RUNTIME !== 'edge') {
    vendoredReactRSC = require('./vendored/rsc/entrypoints');
    vendoredReactSSR = require('./vendored/ssr/entrypoints');
    // In Node environments we need to access the correct React instance from external modules such
    // as global patches. We register the loaded React instances here.
    const { registerServerReact, registerClientReact } = require('../../runtime-reacts.external');
    registerServerReact(vendoredReactRSC.React);
    registerClientReact(vendoredReactSSR.React);
}
export class AppPageRouteModule extends RouteModule {
    match(pathname, prerenderManifest) {
        // Lazily create the matcher based on the provided prerender manifest.
        let matcher = this.matchers.get(prerenderManifest);
        if (!matcher) {
            matcher = new PrerenderManifestMatcher(this.definition.pathname, prerenderManifest);
            this.matchers.set(prerenderManifest, matcher);
        }
        // Match the pathname to the dynamic route.
        return matcher.match(pathname);
    }
    render(req, res, context) {
        return renderToHTMLOrFlight(req, res, context.page, context.query, context.fallbackRouteParams, context.renderOpts, context.serverComponentsHmrCache, context.sharedContext);
    }
    pathCouldBeIntercepted(resolvedPathname, interceptionRoutePatterns) {
        return isInterceptionRouteAppPath(resolvedPathname) || interceptionRoutePatterns.some((regexp)=>{
            return regexp.test(resolvedPathname);
        });
    }
    getVaryHeader(resolvedPathname, interceptionRoutePatterns) {
        const baseVaryHeader = `${RSC_HEADER}, ${NEXT_ROUTER_STATE_TREE_HEADER}, ${NEXT_ROUTER_PREFETCH_HEADER}, ${NEXT_ROUTER_SEGMENT_PREFETCH_HEADER}`;
        if (this.pathCouldBeIntercepted(resolvedPathname, interceptionRoutePatterns)) {
            // Interception route responses can vary based on the `Next-URL` header.
            // We use the Vary header to signal this behavior to the client to properly cache the response.
            return `${baseVaryHeader}, ${NEXT_URL}`;
        } else {
            // We don't need to include `Next-URL` in the Vary header for non-interception routes since it won't affect the response.
            // We also set this header for pages to avoid caching issues when navigating between pages and app.
            return baseVaryHeader;
        }
    }
    constructor(...args){
        super(...args), this.matchers = new WeakMap();
    }
}
const vendored = {
    'react-rsc': vendoredReactRSC,
    'react-ssr': vendoredReactSSR,
    contexts: vendoredContexts
};
export { renderToHTMLOrFlight, vendored };
export default AppPageRouteModule;

//# sourceMappingURL=module.js.map