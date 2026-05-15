import { type NormalizedAppRoute } from '../shared/lib/router/routes/app';
/**
 * Validates that app paths don't create ambiguous routes.
 *
 * NOTE: The paths passed to this function should already have been normalized by normalizeAppPath,
 * which means parallel route segments (@modal, @sidebar, etc.) have been removed.
 *
 * This function performs two types of validation:
 * 1. Individual path validation (syntax, slug names, catch-all placement, etc.)
 * 2. Cross-path validation (ambiguous routes, conflicting patterns)
 *
 * @param appPaths - Array of normalized app router paths to validate
 * @returns Array of validated routes
 * @throws Error if validation fails
 */
export declare function validateAppPaths(appPaths: readonly string[]): NormalizedAppRoute[];
