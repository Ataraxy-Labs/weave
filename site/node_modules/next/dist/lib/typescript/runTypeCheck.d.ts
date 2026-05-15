export interface TypeCheckResult {
    hasWarnings: boolean;
    warnings?: string[];
    inputFilesCount: number;
    totalFilesCount: number;
    incremental: boolean;
}
export interface TypeCheckDirs {
    app?: string;
    pages?: string;
}
export interface DebugBuildPaths {
    app?: string[];
    pages?: string[];
}
export declare function runTypeCheck(typescript: typeof import('typescript'), baseDir: string, distDir: string, tsConfigPath: string, cacheDir?: string, isAppDirEnabled?: boolean, isolatedDevBuild?: boolean, dirs?: TypeCheckDirs, debugBuildPaths?: DebugBuildPaths): Promise<TypeCheckResult>;
