export declare function useRestartServer(): {
    restartServer: ({ invalidateFileSystemCache, }: {
        invalidateFileSystemCache: boolean;
    }) => Promise<void>;
    isPending: boolean;
};
