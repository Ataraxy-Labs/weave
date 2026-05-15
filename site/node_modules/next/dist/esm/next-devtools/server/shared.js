import { codeFrameColumns } from 'next/dist/compiled/babel/code-frame';
import isInternal from '../../shared/lib/is-internal';
import { ignoreListAnonymousStackFramesIfSandwiched as ignoreListAnonymousStackFramesIfSandwichedGeneric } from '../../server/lib/source-maps';
export function ignoreListAnonymousStackFramesIfSandwiched(responses) {
    ignoreListAnonymousStackFramesIfSandwichedGeneric(responses, (response)=>{
        return response.status === 'fulfilled' && response.value.originalStackFrame !== null && response.value.originalStackFrame.file === '<anonymous>';
    }, (response)=>{
        return response.status === 'fulfilled' && response.value.originalStackFrame !== null && response.value.originalStackFrame.ignored === true;
    }, (response)=>{
        return response.status === 'fulfilled' && response.value.originalStackFrame !== null ? response.value.originalStackFrame.methodName : '';
    }, (response)=>{
        ;
        response.value.originalStackFrame.ignored = true;
    });
}
/**
 * It looks up the code frame of the traced source.
 * @note It ignores Next.js/React internals, as these can often be huge bundled files.
 */ export function getOriginalCodeFrame(frame, source, colors = process.stdout.isTTY) {
    if (!source || isInternal(frame.file)) {
        return null;
    }
    return codeFrameColumns(source, {
        start: {
            // 1-based, but -1 means start line without highlighting
            line: frame.line1 ?? -1,
            // 1-based, but 0 means whole line without column highlighting
            column: frame.column1 ?? 0
        }
    }, {
        forceColor: colors
    });
}

//# sourceMappingURL=shared.js.map