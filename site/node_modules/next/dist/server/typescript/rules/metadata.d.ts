import type tsModule from 'typescript/lib/tsserverlibrary';
declare const metadata: {
    client: {
        getSemanticDiagnosticsForExportVariableStatement(fileName: string, node: tsModule.VariableStatement | tsModule.FunctionDeclaration): tsModule.Diagnostic[];
        getSemanticDiagnosticsForExportDeclaration(fileName: string, node: tsModule.ExportDeclaration): tsModule.Diagnostic[];
    };
    server: {
        getSemanticDiagnosticsForExportVariableStatement(fileName: string, node: tsModule.VariableStatement | tsModule.FunctionDeclaration): tsModule.Diagnostic[];
        getSemanticDiagnosticsForExportDeclaration(fileName: string, node: tsModule.ExportDeclaration): tsModule.Diagnostic[];
    };
};
export default metadata;
