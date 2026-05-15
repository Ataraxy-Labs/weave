import { getErrorSource } from '../../../../shared/lib/error-source';
// Dependency injection for stack frame resolver
let stackFrameResolver;
export function setStackFrameResolver(fn) {
    stackFrameResolver = fn;
}
async function resolveStackFrames(request) {
    if (!stackFrameResolver) {
        throw Object.defineProperty(new Error('Stack frame resolver not initialized. This is a bug in Next.js.'), "__NEXT_ERROR_CODE", {
            value: "E822",
            enumerable: false,
            configurable: true
        });
    }
    return stackFrameResolver(request);
}
const formatStackFrame = (frame)=>{
    const file = frame.file || '<unknown>';
    const method = frame.methodName || '<anonymous>';
    const { line1: line, column1: column } = frame;
    return line && column ? `  at ${method} (${file}:${line}:${column})` : line ? `  at ${method} (${file}:${line})` : `  at ${method} (${file})`;
};
const formatErrorFrames = async (frames, context)=>{
    try {
        const resolvedFrames = await resolveStackFrames({
            frames: frames.map((frame)=>({
                    file: frame.file || null,
                    methodName: frame.methodName || '<anonymous>',
                    arguments: [],
                    line1: frame.line1 || null,
                    column1: frame.column1 || null
                })),
            isServer: context.isServer,
            isEdgeServer: context.isEdgeServer,
            isAppDirectory: context.isAppDirectory
        });
        return resolvedFrames.filter((resolvedFrame)=>{
            var _resolvedFrame_value_originalStackFrame;
            return !(resolvedFrame.status === 'fulfilled' && ((_resolvedFrame_value_originalStackFrame = resolvedFrame.value.originalStackFrame) == null ? void 0 : _resolvedFrame_value_originalStackFrame.ignored));
        }).map((resolvedFrame, j)=>resolvedFrame.status === 'fulfilled' && resolvedFrame.value.originalStackFrame ? formatStackFrame(resolvedFrame.value.originalStackFrame) : formatStackFrame(frames[j])).join('\n') + '\n';
    } catch  {
        return frames.map(formatStackFrame).join('\n') + '\n';
    }
};
async function formatRuntimeError(errors, isAppDirectory) {
    const formatError = async (error, index)=>{
        var _error_error, _error_error1, _error_frames;
        const errorHeader = `\n#### Error ${index + 1} (Type: ${error.type})\n\n`;
        const errorName = ((_error_error = error.error) == null ? void 0 : _error_error.name) || 'Error';
        const errorMsg = ((_error_error1 = error.error) == null ? void 0 : _error_error1.message) || 'Unknown error';
        const errorMessage = `**${errorName}**: ${errorMsg}\n\n`;
        if (!((_error_frames = error.frames) == null ? void 0 : _error_frames.length)) {
            var _error_error2;
            const stack = ((_error_error2 = error.error) == null ? void 0 : _error_error2.stack) || '';
            return errorHeader + errorMessage + (stack ? `\`\`\`\n${stack}\n\`\`\`\n` : '');
        }
        const errorSource = getErrorSource(error.error);
        const frames = await formatErrorFrames(error.frames, {
            isServer: errorSource === 'server',
            isEdgeServer: errorSource === 'edge-server',
            isAppDirectory
        });
        return errorHeader + errorMessage + `\`\`\`\n${frames}\`\`\`\n`;
    };
    const formattedErrors = await Promise.all(errors.map(formatError));
    return '### Runtime Errors\n' + formattedErrors.join('\n---\n');
}
export async function formatErrors(errorsByUrl, nextInstanceErrors = {
    nextConfig: []
}) {
    let output = '';
    // Format Next.js instance errors first (e.g., next.config.js errors)
    if (nextInstanceErrors.nextConfig.length > 0) {
        output += `# Next.js Configuration Errors\n\n`;
        output += `**${nextInstanceErrors.nextConfig.length} error(s) found in next.config**\n\n`;
        nextInstanceErrors.nextConfig.forEach((error, index)=>{
            output += `## Error ${index + 1}\n\n`;
            output += '```\n';
            if (error instanceof Error) {
                output += `${error.name}: ${error.message}\n`;
                if (error.stack) {
                    output += error.stack;
                }
            } else {
                output += String(error);
            }
            output += '\n```\n\n';
        });
        output += '---\n\n';
    }
    // Format browser session errors
    if (errorsByUrl.size > 0) {
        output += `# Found errors in ${errorsByUrl.size} browser session(s)\n\n`;
        for (const [url, overlayState] of errorsByUrl){
            const totalErrorCount = overlayState.errors.length + (overlayState.buildError ? 1 : 0);
            if (totalErrorCount === 0) continue;
            let displayUrl = url;
            try {
                const urlObj = new URL(url);
                displayUrl = urlObj.pathname + urlObj.search + urlObj.hash;
            } catch  {
            // If URL parsing fails, use the original URL
            }
            output += `## Session: ${displayUrl}\n\n`;
            output += `**${totalErrorCount} error(s) found**\n\n`;
            // Build errors
            if (overlayState.buildError) {
                output += '### Build Error\n\n';
                output += '```\n';
                output += overlayState.buildError;
                output += '\n```\n\n';
            }
            // Runtime errors with source-mapped stack traces
            if (overlayState.errors.length > 0) {
                const runtimeErrors = await formatRuntimeError(overlayState.errors, overlayState.routerType === 'app');
                output += runtimeErrors;
                output += '\n';
            }
            output += '---\n\n';
        }
    }
    return output.trim();
}

//# sourceMappingURL=format-errors.js.map