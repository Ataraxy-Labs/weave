/**
 * Gets the glob patterns for type definition directories in tsconfig.
 * When isolatedDevBuild is enabled, Next.js uses different distDir paths:
 * - Development: "{distDir}/dev"
 * - Production: "{distDir}"
 */
export declare function getTypeDefinitionGlobPatterns(distDir: string, isolatedDevBuild: boolean): string[];
/**
 * Gets the absolute path to the dev types directory for filtering during type-checking.
 * Returns null if isolatedDevBuild is disabled or in dev mode (where dev types are the main types).
 */
export declare function getDevTypesPath(baseDir: string, distDir: string, isolatedDevBuild: boolean): string | null;
