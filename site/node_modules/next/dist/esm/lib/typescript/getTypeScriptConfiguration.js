import { bold, cyan } from '../picocolors';
import os from 'os';
import path from 'path';
import { FatalError } from '../fatal-error';
import isError from '../is-error';
export async function getTypeScriptConfiguration(typescript, tsConfigPath, metaOnly) {
    try {
        var _result_errors;
        const formatDiagnosticsHost = {
            getCanonicalFileName: (fileName)=>fileName,
            getCurrentDirectory: typescript.sys.getCurrentDirectory,
            getNewLine: ()=>os.EOL
        };
        const { config, error } = typescript.readConfigFile(tsConfigPath, typescript.sys.readFile);
        if (error) {
            throw Object.defineProperty(new FatalError(typescript.formatDiagnostic(error, formatDiagnosticsHost)), "__NEXT_ERROR_CODE", {
                value: "E394",
                enumerable: false,
                configurable: true
            });
        }
        let configToParse = config;
        const result = typescript.parseJsonConfigFileContent(configToParse, // When only interested in meta info,
        // avoid enumerating all files (for performance reasons)
        metaOnly ? {
            ...typescript.sys,
            readDirectory (_path, extensions, _excludes, _includes, _depth) {
                return [
                    extensions ? `file${extensions[0]}` : `file.ts`
                ];
            }
        } : typescript.sys, path.dirname(tsConfigPath));
        if (result.errors) {
            result.errors = result.errors.filter(({ code })=>// No inputs were found in config file
                code !== 18003);
        }
        if ((_result_errors = result.errors) == null ? void 0 : _result_errors.length) {
            throw Object.defineProperty(new FatalError(typescript.formatDiagnostic(result.errors[0], formatDiagnosticsHost)), "__NEXT_ERROR_CODE", {
                value: "E394",
                enumerable: false,
                configurable: true
            });
        }
        return result;
    } catch (err) {
        if (isError(err) && err.name === 'SyntaxError') {
            const reason = '\n' + (err.message ?? '');
            throw Object.defineProperty(new FatalError(bold('Could not parse' + cyan('tsconfig.json') + '.' + ' Please make sure it contains syntactically correct JSON.') + reason), "__NEXT_ERROR_CODE", {
                value: "E339",
                enumerable: false,
                configurable: true
            });
        }
        throw err;
    }
}

//# sourceMappingURL=getTypeScriptConfiguration.js.map