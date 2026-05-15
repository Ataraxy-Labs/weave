interface ResolvedBuildPaths {
    appPaths: string[];
    pagePaths: string[];
}
/**
 * Resolves glob patterns and explicit paths to actual file paths.
 * Categorizes them into App Router and Pages Router paths.
 */
export declare function resolveBuildPaths(patterns: string[], projectDir: string): Promise<ResolvedBuildPaths>;
/**
 * Parse build paths from comma-separated format
 * Supports:
 * - Comma-separated values: "app/page.tsx,app/about/page.tsx"
 *
 * @param input - String input to parse
 * @returns Array of path patterns
 */
export declare function parseBuildPathsInput(input: string): string[];
export {};
