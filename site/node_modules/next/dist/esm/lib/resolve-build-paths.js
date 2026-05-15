import { promisify } from 'util';
import globOriginal from 'next/dist/compiled/glob';
import * as Log from '../build/output/log';
import path from 'path';
import fs from 'fs';
import isError from './is-error';
const glob = promisify(globOriginal);
/**
 * Escapes bracket expressions that correspond to existing directories.
 * This allows Next.js dynamic routes like [slug] to work with glob patterns.
 *
 * e.g., "app/blog/[slug]/** /page.tsx" → "app/blog/\[slug\]/** /page.tsx"
 *       (if app/blog/[slug] directory exists)
 */ function escapeExistingBrackets(pattern, projectDir) {
    // Match bracket expressions: [name], [...name], [[...name]]
    const bracketRegex = /\[\[?\.\.\.[^\]]+\]?\]|\[[^\]]+\]/g;
    let lastIndex = 0;
    let result = '';
    let match;
    while((match = bracketRegex.exec(pattern)) !== null){
        const pathPrefix = pattern.slice(0, match.index + match[0].length);
        const exists = fs.existsSync(path.join(projectDir, pathPrefix));
        result += pattern.slice(lastIndex, match.index);
        result += exists ? match[0].replace(/\[/g, '\\[').replace(/\]/g, '\\]') : match[0];
        lastIndex = match.index + match[0].length;
    }
    return result + pattern.slice(lastIndex);
}
/**
 * Resolves glob patterns and explicit paths to actual file paths.
 * Categorizes them into App Router and Pages Router paths.
 */ export async function resolveBuildPaths(patterns, projectDir) {
    const appPaths = new Set();
    const pagePaths = new Set();
    for (const pattern of patterns){
        const trimmed = pattern.trim();
        if (!trimmed) continue;
        try {
            // Escape brackets that correspond to existing Next.js dynamic route directories
            const escapedPattern = escapeExistingBrackets(trimmed, projectDir);
            const matches = await glob(escapedPattern, {
                cwd: projectDir
            });
            if (matches.length === 0) {
                Log.warn(`Pattern "${trimmed}" did not match any files`);
            }
            for (const file of matches){
                if (!fs.statSync(path.join(projectDir, file)).isDirectory()) {
                    categorizeAndAddPath(file, appPaths, pagePaths);
                }
            }
        } catch (error) {
            throw Object.defineProperty(new Error(`Failed to resolve pattern "${trimmed}": ${isError(error) ? error.message : String(error)}`), "__NEXT_ERROR_CODE", {
                value: "E972",
                enumerable: false,
                configurable: true
            });
        }
    }
    return {
        appPaths: Array.from(appPaths).sort(),
        pagePaths: Array.from(pagePaths).sort()
    };
}
/**
 * Categorizes a file path to either app or pages router based on its prefix.
 *
 * Examples:
 * - "app/page.tsx" → appPaths.add("/page.tsx")
 * - "pages/index.tsx" → pagePaths.add("/index.tsx")
 */ function categorizeAndAddPath(filePath, appPaths, pagePaths) {
    const normalized = filePath.replace(/\\/g, '/');
    if (normalized.startsWith('app/')) {
        appPaths.add('/' + normalized.slice(4));
    } else if (normalized.startsWith('pages/')) {
        pagePaths.add('/' + normalized.slice(6));
    }
}
/**
 * Parse build paths from comma-separated format
 * Supports:
 * - Comma-separated values: "app/page.tsx,app/about/page.tsx"
 *
 * @param input - String input to parse
 * @returns Array of path patterns
 */ export function parseBuildPathsInput(input) {
    // Comma-separated values
    return input.split(',').map((p)=>p.trim()).filter((p)=>p.length > 0);
}

//# sourceMappingURL=resolve-build-paths.js.map