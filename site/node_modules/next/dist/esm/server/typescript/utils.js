import path from 'path';
let ts;
let info;
let appDirRegExp;
export function log(message) {
    info.project.projectService.logger.info('[next] ' + message);
}
// This function has to be called initially.
export function init(opts) {
    const projectDir = opts.info.project.getCurrentDirectory();
    ts = opts.ts;
    info = opts.info;
    appDirRegExp = new RegExp('^' + (projectDir + '(/src)?/app').replace(/[\\/]/g, '[\\/]'));
    log('Initialized Next.js TypeScript plugin: ' + projectDir);
}
export function getTs() {
    return ts;
}
export function getInfo() {
    return info;
}
export function getTypeChecker() {
    const program = info.languageService.getProgram();
    if (!program) {
        log('Failed to get program while while running getTypeChecker.');
        return;
    }
    const typeChecker = program.getTypeChecker();
    if (!typeChecker) {
        log('Failed to get type checker while running getTypeChecker.');
        return;
    }
    return typeChecker;
}
export function getSource(fileName) {
    const program = info.languageService.getProgram();
    if (!program) {
        log('Failed to get program while running getSource for: ' + fileName);
        return;
    }
    const sourceFile = program.getSourceFile(fileName);
    if (!sourceFile) {
        log('Failed to get source file while running getSource for: ' + fileName);
        return;
    }
    return sourceFile;
}
export function removeStringQuotes(str) {
    return str.replace(/^['"`]|['"`]$/g, '');
}
export const isPositionInsideNode = (position, node)=>{
    const start = node.getFullStart();
    return start <= position && position <= node.getFullWidth() + start;
};
export const isDefaultFunctionExport = (node)=>{
    if (ts.isFunctionDeclaration(node)) {
        let hasExportKeyword = false;
        let hasDefaultKeyword = false;
        if (node.modifiers) {
            for (const modifier of node.modifiers){
                if (modifier.kind === ts.SyntaxKind.ExportKeyword) {
                    hasExportKeyword = true;
                } else if (modifier.kind === ts.SyntaxKind.DefaultKeyword) {
                    hasDefaultKeyword = true;
                }
            }
        }
        // `export default function`
        if (hasExportKeyword && hasDefaultKeyword) {
            return true;
        }
    }
    return false;
};
export const isInsideApp = (filePath)=>{
    return appDirRegExp.test(filePath);
};
export const isAppEntryFile = (filePath)=>{
    return appDirRegExp.test(filePath) && /^(page|layout)\.(mjs|js|jsx|ts|tsx)$/.test(path.basename(filePath));
};
export const isPageFile = (filePath)=>{
    return appDirRegExp.test(filePath) && /^page\.(mjs|js|jsx|ts|tsx)$/.test(path.basename(filePath));
};
// Check if a module is a client entry.
export function getEntryInfo(fileName, throwOnInvalidDirective) {
    const source = getSource(fileName);
    if (source) {
        let isDirective = true;
        let isClientEntry = false;
        let isServerEntry = false;
        ts.forEachChild(source, (node)=>{
            if (ts.isExpressionStatement(node) && ts.isStringLiteral(node.expression)) {
                if (node.expression.text === 'use client') {
                    if (isDirective) {
                        isClientEntry = true;
                    } else {
                        if (throwOnInvalidDirective) {
                            const e = {
                                messageText: 'The `"use client"` directive must be put at the top of the file.',
                                start: node.expression.getStart(),
                                length: node.expression.getWidth()
                            };
                            throw e;
                        }
                    }
                } else if (node.expression.text === 'use server') {
                    if (isDirective) {
                        isServerEntry = true;
                    } else {
                        if (throwOnInvalidDirective) {
                            const e = {
                                messageText: 'The `"use server"` directive must be put at the top of the file.',
                                start: node.expression.getStart(),
                                length: node.expression.getWidth()
                            };
                            throw e;
                        }
                    }
                }
                if (isClientEntry && isServerEntry) {
                    const e = {
                        messageText: 'Cannot use both "use client" and "use server" directives in the same file.',
                        start: node.expression.getStart(),
                        length: node.expression.getWidth()
                    };
                    throw e;
                }
            } else {
                isDirective = false;
            }
        });
        return {
            client: isClientEntry,
            server: isServerEntry
        };
    }
    return {
        client: false,
        server: false
    };
}

//# sourceMappingURL=utils.js.map