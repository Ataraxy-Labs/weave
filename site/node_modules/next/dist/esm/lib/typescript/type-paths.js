import path from 'path';
/**
 * Gets the glob patterns for type definition directories in tsconfig.
 * When isolatedDevBuild is enabled, Next.js uses different distDir paths:
 * - Development: "{distDir}/dev"
 * - Production: "{distDir}"
 */ export function getTypeDefinitionGlobPatterns(distDir, isolatedDevBuild) {
    const distDirPosix = path.win32.sep === path.sep ? distDir.replaceAll(path.win32.sep, path.posix.sep) : distDir;
    const typeGlobPatterns = [
        `${distDirPosix}/types/**/*.ts`
    ];
    // When isolatedDevBuild is enabled, include both .next/types and .next/dev/types
    // to avoid tsconfig churn when switching between dev/build modes
    if (isolatedDevBuild) {
        typeGlobPatterns.push(process.env.NODE_ENV === 'development' ? `${distDirPosix.replace(/\/dev$/, '')}/types/**/*.ts` : `${distDirPosix}/dev/types/**/*.ts`);
        // Sort for consistent order
        typeGlobPatterns.sort((a, b)=>a.length - b.length);
    }
    return typeGlobPatterns;
}
/**
 * Gets the absolute path to the dev types directory for filtering during type-checking.
 * Returns null if isolatedDevBuild is disabled or in dev mode (where dev types are the main types).
 */ export function getDevTypesPath(baseDir, distDir, isolatedDevBuild) {
    if (!isolatedDevBuild) {
        return null;
    }
    const isDev = process.env.NODE_ENV === 'development';
    if (isDev) {
        // In dev mode, dev types are the main types, so no need to filter
        return null;
    }
    // In build mode, dev types are at "{baseDir}/{distDir}/dev/types" and should be filtered
    return path.join(baseDir, distDir, 'dev', 'types');
}

//# sourceMappingURL=type-paths.js.map