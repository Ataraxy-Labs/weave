/**
 * Needs to be in the same error boundary as the shell.
 * If it commits, we know we recovered from an SSR error.
 * If it doesn't commit, we errored again and React will take care of error reporting.
 */
export declare function ReplaySsrOnlyErrors({ onBlockingError, }: {
    onBlockingError: () => void;
}): null;
