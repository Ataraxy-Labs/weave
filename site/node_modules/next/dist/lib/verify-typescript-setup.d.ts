import type { TypeCheckResult } from './typescript/runTypeCheck';
export declare function verifyTypeScriptSetup({ dir, distDir, cacheDir, tsconfigPath, typeCheckPreflight, disableStaticImages, hasAppDir, hasPagesDir, isolatedDevBuild, appDir, pagesDir, debugBuildPaths, }: {
    dir: string;
    distDir: string;
    cacheDir?: string;
    tsconfigPath: string | undefined;
    typeCheckPreflight: boolean;
    disableStaticImages: boolean;
    hasAppDir: boolean;
    hasPagesDir: boolean;
    isolatedDevBuild: boolean | undefined;
    appDir?: string;
    pagesDir?: string;
    debugBuildPaths?: {
        app?: string[];
        pages?: string[];
    };
}): Promise<{
    result?: TypeCheckResult;
    version: string | null;
}>;
