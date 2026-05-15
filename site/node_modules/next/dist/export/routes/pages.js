"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "exportPagesPage", {
    enumerable: true,
    get: function() {
        return exportPagesPage;
    }
});
const _renderresult = /*#__PURE__*/ _interop_require_default(require("../../server/render-result"));
const _path = require("path");
const _constants = require("../../lib/constants");
const _bailouttocsr = require("../../shared/lib/lazy-dynamic/bailout-to-csr");
const _modulerender = require("../../server/route-modules/pages/module.render");
function _interop_require_default(obj) {
    return obj && obj.__esModule ? obj : {
        default: obj
    };
}
async function exportPagesPage(req, res, path, page, query, params, htmlFilepath, htmlFilename, pagesDataDir, buildExport, isDynamic, sharedContext, renderContext, hasOrigQueryValues, renderOpts, components, fileWriter) {
    if (components.getServerSideProps) {
        throw Object.defineProperty(new Error(`Error for page ${page}: ${_constants.SERVER_PROPS_EXPORT_ERROR}`), "__NEXT_ERROR_CODE", {
            value: "E15",
            enumerable: false,
            configurable: true
        });
    }
    // for non-dynamic SSG pages we should have already
    // prerendered the file
    if (!buildExport && components.getStaticProps && !isDynamic) {
        return;
    }
    // Pages router merges page params (e.g. [lang]) with query params
    // primarily to support them both being accessible on `useRouter().query`.
    // If we extracted dynamic params from the path, we need to merge them
    // back into the query object.
    const searchAndDynamicParams = {
        ...query,
        ...params
    };
    if (components.getStaticProps && !htmlFilepath.endsWith('.html')) {
        // make sure it ends with .html if the name contains a dot
        htmlFilepath += '.html';
        htmlFilename += '.html';
    }
    let renderResult;
    if (typeof components.Component === 'string') {
        renderResult = _renderresult.default.fromStatic(components.Component, _constants.HTML_CONTENT_TYPE_HEADER);
        if (hasOrigQueryValues) {
            throw Object.defineProperty(new Error(`\nError: you provided query values for ${path} which is an auto-exported page. These can not be applied since the page can no longer be re-rendered on the server. To disable auto-export for this page add \`getInitialProps\`\n`), "__NEXT_ERROR_CODE", {
                value: "E505",
                enumerable: false,
                configurable: true
            });
        }
    } else {
        /**
     * This sets environment variable to be used at the time of SSR by head.tsx.
     * Using this from process.env allows targeting SSR by calling
     * `process.env.__NEXT_OPTIMIZE_CSS`.
     */ if (renderOpts.optimizeCss) {
            process.env.__NEXT_OPTIMIZE_CSS = JSON.stringify(true);
        }
        try {
            renderResult = await (0, _modulerender.lazyRenderPagesPage)(req, res, page, searchAndDynamicParams, renderOpts, sharedContext, renderContext);
        } catch (err) {
            if (!(0, _bailouttocsr.isBailoutToCSRError)(err)) throw err;
        }
    }
    const ssgNotFound = renderResult == null ? void 0 : renderResult.metadata.isNotFound;
    const html = renderResult && !renderResult.isNull ? renderResult.toUnchunkedString() : '';
    const metadata = (renderResult == null ? void 0 : renderResult.metadata) || {};
    if (metadata.pageData) {
        const dataFile = (0, _path.join)(pagesDataDir, htmlFilename.replace(/\.html$/, _constants.NEXT_DATA_SUFFIX));
        fileWriter.append(dataFile, JSON.stringify(metadata.pageData));
    }
    if (!ssgNotFound) {
        // don't attempt writing to disk if getStaticProps returned not found
        fileWriter.append(htmlFilepath, html);
    }
    return {
        cacheControl: metadata.cacheControl ?? {
            revalidate: false,
            expire: undefined
        },
        ssgNotFound
    };
}

//# sourceMappingURL=pages.js.map