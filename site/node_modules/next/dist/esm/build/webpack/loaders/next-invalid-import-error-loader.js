const nextInvalidImportErrorLoader = function() {
    const { message } = this.getOptions();
    const error = Object.defineProperty(new Error(message), "__NEXT_ERROR_CODE", {
        value: "E394",
        enumerable: false,
        configurable: true
    });
    if (process.env.NEXT_RSPACK) {
        // Rspack uses miette for error formatting, which automatically includes stack
        // traces in the error message. To avoid showing redundant stack information
        // in the final error output, we clear the stack property.
        error.stack = undefined;
    }
    throw error;
};
export default nextInvalidImportErrorLoader;

//# sourceMappingURL=next-invalid-import-error-loader.js.map