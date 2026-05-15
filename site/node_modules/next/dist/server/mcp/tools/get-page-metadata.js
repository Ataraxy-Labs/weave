"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
0 && (module.exports = {
    handlePageMetadataResponse: null,
    registerGetPageMetadataTool: null
});
function _export(target, all) {
    for(var name in all)Object.defineProperty(target, name, {
        enumerable: true,
        get: all[name]
    });
}
_export(exports, {
    handlePageMetadataResponse: function() {
        return handlePageMetadataResponse;
    },
    registerGetPageMetadataTool: function() {
        return registerGetPageMetadataTool;
    }
});
const _hotreloadertypes = require("../../dev/hot-reloader-types");
const _browsercommunication = require("./utils/browser-communication");
const _mcptelemetrytracker = require("../mcp-telemetry-tracker");
function registerGetPageMetadataTool(server, sendHmrMessage, getActiveConnectionCount) {
    server.registerTool('get_page_metadata', {
        description: 'Get runtime metadata about what contributes to the current page render from active browser sessions.',
        inputSchema: {}
    }, async (_request)=>{
        // Track telemetry
        _mcptelemetrytracker.mcpTelemetryTracker.recordToolCall('mcp/get_page_metadata');
        try {
            const connectionCount = getActiveConnectionCount();
            if (connectionCount === 0) {
                return {
                    content: [
                        {
                            type: 'text',
                            text: 'No browser sessions connected. Please open your application in a browser to retrieve page metadata.'
                        }
                    ]
                };
            }
            const responses = await (0, _browsercommunication.createBrowserRequest)(_hotreloadertypes.HMR_MESSAGE_SENT_TO_BROWSER.REQUEST_PAGE_METADATA, sendHmrMessage, getActiveConnectionCount, _browsercommunication.DEFAULT_BROWSER_REQUEST_TIMEOUT_MS);
            if (responses.length === 0) {
                return {
                    content: [
                        {
                            type: 'text',
                            text: 'No browser sessions responded.'
                        }
                    ]
                };
            }
            const sessionMetadata = [];
            for (const response of responses){
                if (response.data) {
                    // TODO: Add other metadata for the current page render here. Currently, we only have segment trie data.
                    const pageMetadata = convertSegmentTrieToPageMetadata(response.data);
                    sessionMetadata.push({
                        url: response.url,
                        metadata: pageMetadata
                    });
                }
            }
            if (sessionMetadata.length === 0) {
                return {
                    content: [
                        {
                            type: 'text',
                            text: `No page metadata available from ${responses.length} browser session(s).`
                        }
                    ]
                };
            }
            const output = formatPageMetadata(sessionMetadata);
            return {
                content: [
                    {
                        type: 'text',
                        text: output
                    }
                ]
            };
        } catch (error) {
            return {
                content: [
                    {
                        type: 'text',
                        text: `Error: ${error instanceof Error ? error.message : String(error)}`
                    }
                ]
            };
        }
    });
}
function handlePageMetadataResponse(requestId, segmentTrieData, url) {
    (0, _browsercommunication.handleBrowserPageResponse)(requestId, segmentTrieData, url || '');
}
function convertSegmentTrieToPageMetadata(data) {
    const segments = [];
    if (data.segmentTrie) {
        // Traverse the trie and collect all segments
        function traverseTrie(node) {
            if (node.value) {
                segments.push({
                    type: node.value.type,
                    pagePath: node.value.pagePath,
                    boundaryType: node.value.boundaryType
                });
            }
            for (const childNode of Object.values(node.children)){
                if (childNode) {
                    traverseTrie(childNode);
                }
            }
        }
        traverseTrie(data.segmentTrie);
    }
    return {
        segments,
        routerType: data.routerType
    };
}
function formatPageMetadata(sessionMetadata) {
    let output = `# Page metadata from ${sessionMetadata.length} browser session(s)\n\n`;
    for (const { url, metadata } of sessionMetadata){
        let displayUrl = url;
        try {
            const urlObj = new URL(url);
            displayUrl = urlObj.pathname + urlObj.search + urlObj.hash;
        } catch  {
        // If URL parsing fails, use the original URL
        }
        output += `## Session: ${displayUrl}\n\n`;
        output += `**Router type:** ${metadata.routerType}\n\n`;
        if (metadata.segments.length === 0) {
            output += '*No segments found*\n\n';
        } else {
            output += '### Files powering this page:\n\n';
            // Ensure consistent output to avoid flaky tests
            const sortedSegments = [
                ...metadata.segments
            ].sort((a, b)=>{
                const typeOrder = (segment)=>{
                    const type = segment.boundaryType || segment.type;
                    if (type === 'layout') return 0;
                    if (type.startsWith('boundary:')) return 1;
                    if (type === 'page') return 2;
                    return 3;
                };
                const aOrder = typeOrder(a);
                const bOrder = typeOrder(b);
                if (aOrder !== bOrder) return aOrder - bOrder;
                return a.pagePath.localeCompare(b.pagePath);
            });
            for (const segment of sortedSegments){
                const path = segment.pagePath;
                const isBuiltin = path.startsWith('__next_builtin__');
                const type = segment.boundaryType || segment.type;
                const isBoundary = type.startsWith('boundary:');
                let displayPath = path.replace(/@boundary$/, '').replace(/^__next_builtin__/, '');
                if (!isBuiltin && !displayPath.startsWith('app/')) {
                    displayPath = `app/${displayPath}`;
                }
                const descriptors = [];
                if (isBoundary) descriptors.push('boundary');
                if (isBuiltin) descriptors.push('builtin');
                const descriptor = descriptors.length > 0 ? ` (${descriptors.join(', ')})` : '';
                output += `- ${displayPath}${descriptor}\n`;
            }
            output += '\n';
        }
        output += '---\n\n';
    }
    return output.trim();
}

//# sourceMappingURL=get-page-metadata.js.map