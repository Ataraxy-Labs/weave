import { getOriginalStackFrames as getOriginalStackFramesWebpack } from '../middleware-webpack';
import { getOriginalStackFrames as getOriginalStackFramesTurbopack } from '../middleware-turbopack';
import { dim } from '../../../lib/picocolors';
import { parseStack } from '../../lib/parse-stack';
import path from 'path';
import { LRUCache } from '../../lib/lru-cache';
// TODO: handle server vs browser error source mapping correctly
export async function mapFramesUsingBundler(frames, ctx) {
    switch(ctx.bundler){
        case 'webpack':
            {
                const { isServer, isEdgeServer, isAppDirectory, clientStats, serverStats, edgeServerStats, rootDirectory } = ctx;
                const res = await getOriginalStackFramesWebpack({
                    isServer,
                    isEdgeServer,
                    isAppDirectory,
                    frames,
                    clientStats,
                    serverStats,
                    edgeServerStats,
                    rootDirectory
                });
                return res;
            }
        case 'turbopack':
            {
                const { project, projectPath, isServer, isEdgeServer, isAppDirectory } = ctx;
                const res = await getOriginalStackFramesTurbopack({
                    project,
                    projectPath,
                    frames,
                    isServer,
                    isEdgeServer,
                    isAppDirectory
                });
                return res;
            }
        default:
            {
                return null;
            }
    }
}
// converts _next/static/chunks/... to file:///.next/static/chunks/... for parseStack
// todo: where does next dev overlay handle this case and re-use that logic
function preprocessStackTrace(stackTrace, distDir) {
    return stackTrace.split('\n').map((line)=>{
        const match = line.match(/^(\s*at\s+.*?)\s+\(([^)]+)\)$/);
        if (match) {
            const [, prefix, location] = match;
            if (location.startsWith('_next/static/') && distDir) {
                const normalizedDistDir = distDir.replace(/\\/g, '/').replace(/\/$/, '');
                const absolutePath = normalizedDistDir + '/' + location.slice('_next/'.length);
                const fileUrl = `file://${path.resolve(absolutePath)}`;
                return `${prefix} (${fileUrl})`;
            }
        }
        return line;
    }).join('\n');
}
const cache = new LRUCache(25);
async function getSourceMappedStackFramesInternal(stackTrace, ctx, distDir, ignore = true) {
    try {
        var _filteredFrames_find;
        const normalizedStack = preprocessStackTrace(stackTrace, distDir);
        const frames = parseStack(normalizedStack, distDir);
        if (frames.length === 0) {
            return {
                kind: 'stack',
                stack: stackTrace
            };
        }
        const mappingResults = await mapFramesUsingBundler(frames, ctx);
        const processedFrames = mappingResults.map((result, index)=>({
                result,
                originalFrame: frames[index]
            })).map(({ result, originalFrame })=>{
            var _originalStackFrame_file;
            if (result.status === 'rejected') {
                return {
                    kind: 'rejected',
                    frameText: formatStackFrame(originalFrame),
                    codeFrame: null
                };
            }
            const { originalStackFrame, originalCodeFrame } = result.value;
            if ((originalStackFrame == null ? void 0 : originalStackFrame.ignored) && ignore) {
                return {
                    kind: 'ignored'
                };
            }
            // should we apply this generally to dev overlay (dev overlay does not ignore chrome-extension://)
            if (originalStackFrame == null ? void 0 : (_originalStackFrame_file = originalStackFrame.file) == null ? void 0 : _originalStackFrame_file.startsWith('chrome-extension://')) {
                return {
                    kind: 'ignored'
                };
            }
            return {
                kind: 'success',
                // invariant: if result is not rejected and not ignored, then original stack frame exists
                // verifiable by tracing `getOriginalStackFrame`. The invariant exists because of bad types
                frameText: formatStackFrame(originalStackFrame),
                codeFrame: originalCodeFrame
            };
        });
        const allIgnored = processedFrames.every((frame)=>frame.kind === 'ignored');
        // we want to handle **all** ignored vs all/some rejected differently
        // if all are ignored we should show no frames
        // if all are rejected, we want to fallback to showing original stack frames
        if (allIgnored) {
            return {
                kind: 'all-ignored'
            };
        }
        const filteredFrames = processedFrames.filter((frame)=>frame.kind !== 'ignored');
        if (filteredFrames.length === 0) {
            return {
                kind: 'stack',
                stack: stackTrace
            };
        }
        const stackOutput = filteredFrames.map((frame)=>frame.frameText).join('\n');
        const firstFrameCode = (_filteredFrames_find = filteredFrames.find((frame)=>frame.codeFrame)) == null ? void 0 : _filteredFrames_find.codeFrame;
        if (firstFrameCode) {
            return {
                kind: 'with-frame-code',
                frameCode: firstFrameCode,
                stack: stackOutput,
                frames: filteredFrames
            };
        }
        // i don't think this a real case, but good for exhaustion
        return {
            kind: 'mapped-stack',
            stack: stackOutput,
            frames: filteredFrames
        };
    } catch (error) {
        return {
            kind: 'stack',
            stack: stackTrace
        };
    }
}
// todo: cache the actual async call, not the wrapper with post processing
export async function getSourceMappedStackFrames(stackTrace, ctx, distDir, ignore = true) {
    const cacheKey = `sm_${stackTrace}-${ctx.bundler}-${ctx.isAppDirectory}-${ctx.isEdgeServer}-${ctx.isServer}-${distDir}-${ignore}`;
    const cacheItem = cache.get(cacheKey);
    if (cacheItem) {
        return cacheItem;
    }
    const result = await getSourceMappedStackFramesInternal(stackTrace, ctx, distDir, ignore);
    cache.set(cacheKey, result);
    return result;
}
function formatStackFrame(frame) {
    const functionName = frame.methodName || '<anonymous>';
    const location = frame.file && frame.line1 ? `${frame.file}:${frame.line1}${frame.column1 ? `:${frame.column1}` : ''}` : frame.file || '<unknown>';
    return `    at ${functionName} (${location})`;
}
// appends the source mapped location of the console method
export const withLocation = async ({ original, stack }, ctx, distDir, config)=>{
    if (typeof config === 'object' && config.showSourceLocation === false) {
        return original;
    }
    if (!stack) {
        return original;
    }
    const res = await getSourceMappedStackFrames(stack, ctx, distDir);
    const location = getConsoleLocation(res);
    if (!location) {
        return original;
    }
    return [
        ...original,
        dim(`(${location})`)
    ];
};
export const getConsoleLocation = (mapped)=>{
    if (mapped.kind !== 'mapped-stack' && mapped.kind !== 'with-frame-code') {
        return null;
    }
    const first = mapped.frames.at(0);
    if (!first) {
        return null;
    }
    // we don't want to show the name of parent function (at <fn> thing in stack), just source location for minimal noise
    const match = first.frameText.match(/\(([^)]+)\)/);
    const locationText = match ? match[1] : first.frameText;
    return locationText;
};

//# sourceMappingURL=source-map.js.map