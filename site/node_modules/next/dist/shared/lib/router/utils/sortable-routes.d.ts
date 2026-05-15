/**
 * A route that can be sorted by specificity.
 */
export type SortableRoute = {
    /**
     * The source page of the route. This represents the original page that's on
     * disk. For example, the `app/[lang]/[...rest]/page.tsx` would have a source
     * page of '/[lang]/[...rest]'.
     */
    readonly sourcePage: string;
    /**
     * The page of the route. This represents the final rendered route. For
     * example, the `app/[lang]/[...rest]/page.tsx` that was rendered with a lang
     * value of `en` would have a page of '/en/[...rest]'.
     */
    readonly page: string;
};
/**
 * Determines the specificity of a route segment for sorting purposes.
 *
 * In Next.js routing, more specific routes should match before less specific ones.
 * This function returns a numeric value where lower numbers indicate higher specificity.
 *
 * Specificity order (most to least specific):
 * 1. Static segments (e.g., "about", "api") - return 0
 * 2. Dynamic segments (e.g., "[id]", "[slug]") - return 1
 * 3. Catch-all segments (e.g., "[...slug]") - return 2
 * 4. Optional catch-all segments (e.g., "[[...slug]]") - return 3
 *
 * @param segment - A single path segment (e.g., "api", "[id]", "[...slug]")
 * @returns A numeric specificity value (0-3, where 0 is most specific)
 */
export declare function getSegmentSpecificity(segment: string): number;
/**
 * Compares two route paths using a depth-first traversal approach.
 *
 * This function implements a deterministic comparison that sorts routes by specificity:
 * 1. More specific routes come first (fewer dynamic segments)
 * 2. Shorter routes are more specific than longer ones
 * 3. Routes with same specificity are sorted lexicographically
 *
 * The comparison is done segment by segment, left to right, similar to how
 * you would traverse a route tree in depth-first order.
 *
 * @param pathA - First route path to compare (e.g., "/api/users/[id]")
 * @param pathB - Second route path to compare (e.g., "/api/[...slug]")
 * @returns Negative if pathA is more specific, positive if pathB is more specific, 0 if equal
 */
export declare function compareRouteSegments(pathA: string, pathB: string): number;
/**
 * Sorts an array of routes by specificity using a deterministic depth-first traversal approach.
 *
 * This function implements Next.js route matching priority where more specific routes
 * should be matched before less specific ones. The sorting is deterministic and stable,
 * meaning identical inputs will always produce identical outputs.
 *
 * Sorting criteria (in order of priority):
 * 1. Source path specificity (primary)
 * 2. Page path specificity (secondary)
 * 3. Lexicographic ordering (tertiary, for determinism)
 *
 * Examples of specificity order:
 * - "/api/users" (static) comes before "/api/[slug]" (dynamic)
 * - "/api/[id]" (dynamic) comes before "/api/[...slug]" (catch-all)
 * - "/api/[...slug]" (catch-all) comes before "/api/[[...slug]]" (optional catch-all)
 *
 * @param routes - Array of routes to sort
 * @returns New sorted array (does not mutate input)
 */
export declare function sortSortableRoutes(routes: readonly SortableRoute[]): readonly SortableRoute[];
/**
 * Sorts an array of pages by specificity using a deterministic depth-first
 * traversal approach.
 *
 * @param pages - Array of pages to sort
 * @returns New sorted array (does not mutate input)
 */
export declare function sortPages(pages: readonly string[]): readonly string[];
/**
 * Sorts an array of objects by sourcePage and page using a deterministic
 * depth-first traversal approach.
 *
 * @param objects - Array of objects to sort
 * @param getter - Function to get the sourcePage and page from an object
 * @returns New sorted array (does not mutate input)
 */
export declare function sortSortableRouteObjects<T>(objects: readonly T[], getter: (object: T) => SortableRoute): readonly T[];
/**
 * Sorts an array of objects by page using a deterministic depth-first traversal
 * approach.
 *
 * @param objects - Array of objects to sort
 * @param getter - Function to get the page from an object
 * @returns New sorted array (does not mutate input)
 */
export declare function sortPageObjects<T>(objects: readonly T[], getter: (object: T) => string): readonly T[];
