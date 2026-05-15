export declare function isReactLargeShellError(error: unknown): error is Error & {
    digest?: string;
};
