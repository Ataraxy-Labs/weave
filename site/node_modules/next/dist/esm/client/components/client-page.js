'use client';
import { jsx as _jsx } from "react/jsx-runtime";
import { InvariantError } from '../../shared/lib/invariant-error';
import { LayoutRouterContext } from '../../shared/lib/app-router-context.shared-runtime';
import { use } from 'react';
import { urlSearchParamsToParsedUrlQuery } from '../route-params';
import { SearchParamsContext } from '../../shared/lib/hooks-client-context.shared-runtime';
/**
 * When the Page is a client component we send the params and searchParams to this client wrapper
 * where they are turned into dynamically tracked values before being passed to the actual Page component.
 *
 * additionally we may send promises representing the params and searchParams. We don't ever use these passed
 * values but it can be necessary for the sender to send a Promise that doesn't resolve in certain situations.
 * It is up to the caller to decide if the promises are needed.
 */ export function ClientPageRoot({ Component, serverProvidedParams }) {
    let searchParams;
    let params;
    if (serverProvidedParams !== null) {
        searchParams = serverProvidedParams.searchParams;
        params = serverProvidedParams.params;
    } else {
        // When Cache Components is enabled, the server does not pass the params as
        // props; they are parsed on the client and passed via context.
        const layoutRouterContext = use(LayoutRouterContext);
        params = layoutRouterContext !== null ? layoutRouterContext.parentParams : {};
        // This is an intentional behavior change: when Cache Components is enabled,
        // client segments receive the "canonical" search params, not the
        // rewritten ones. Users should either call useSearchParams directly or pass
        // the rewritten ones in from a Server Component.
        // TODO: Log a deprecation error when this object is accessed
        searchParams = urlSearchParamsToParsedUrlQuery(use(SearchParamsContext));
    }
    if (typeof window === 'undefined') {
        const { workAsyncStorage } = require('../../server/app-render/work-async-storage.external');
        let clientSearchParams;
        let clientParams;
        // We are going to instrument the searchParams prop with tracking for the
        // appropriate context. We wrap differently in prerendering vs rendering
        const store = workAsyncStorage.getStore();
        if (!store) {
            throw Object.defineProperty(new InvariantError('Expected workStore to exist when handling searchParams in a client Page.'), "__NEXT_ERROR_CODE", {
                value: "E564",
                enumerable: false,
                configurable: true
            });
        }
        const { createSearchParamsFromClient } = require('../../server/request/search-params');
        clientSearchParams = createSearchParamsFromClient(searchParams, store);
        const { createParamsFromClient } = require('../../server/request/params');
        clientParams = createParamsFromClient(params, store);
        return /*#__PURE__*/ _jsx(Component, {
            params: clientParams,
            searchParams: clientSearchParams
        });
    } else {
        const { createRenderSearchParamsFromClient } = require('../request/search-params.browser');
        const clientSearchParams = createRenderSearchParamsFromClient(searchParams);
        const { createRenderParamsFromClient } = require('../request/params.browser');
        const clientParams = createRenderParamsFromClient(params);
        return /*#__PURE__*/ _jsx(Component, {
            params: clientParams,
            searchParams: clientSearchParams
        });
    }
}

//# sourceMappingURL=client-page.js.map