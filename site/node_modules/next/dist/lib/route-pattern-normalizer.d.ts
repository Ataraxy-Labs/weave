import type { Token } from 'next/dist/compiled/path-to-regexp';
/**
 * Route pattern normalization utilities for path-to-regexp compatibility.
 *
 * path-to-regexp 6.3.0+ introduced stricter validation that rejects certain
 * patterns commonly used in Next.js interception routes. This module provides
 * normalization functions to make Next.js route patterns compatible with the
 * updated library while preserving all functionality.
 */
/**
 * Internal separator used to normalize adjacent parameter patterns.
 * This unique marker is inserted between adjacent parameters and stripped out
 * during parameter extraction to avoid conflicts with real URL content.
 */
export declare const PARAM_SEPARATOR = "_NEXTSEP_";
/**
 * Detects if a route pattern needs normalization for path-to-regexp compatibility.
 */
export declare function hasAdjacentParameterIssues(route: string): boolean;
/**
 * Normalizes route patterns that have adjacent parameters without text between them.
 * Inserts a unique separator that can be safely stripped out later.
 */
export declare function normalizeAdjacentParameters(route: string): string;
/**
 * Normalizes tokens that have repeating modifiers (* or +) but empty prefix and suffix.
 *
 * path-to-regexp 6.3.0+ introduced validation that throws:
 * "Can not repeat without prefix/suffix"
 *
 * This occurs when a token has modifier: '*' or '+' with both prefix: '' and suffix: ''
 */
export declare function normalizeTokensForRegexp(tokens: Token[]): Token[];
/**
 * Strips normalization separators from compiled pathname.
 * This removes separators that were inserted by normalizeAdjacentParameters
 * to satisfy path-to-regexp validation.
 *
 * Only removes separators in the specific contexts where they were inserted:
 * - After interception route markers: (.)_NEXTSEP_ -> (.)
 *
 * This targeted approach ensures we don't accidentally remove the separator
 * from legitimate user content.
 */
export declare function stripNormalizedSeparators(pathname: string): string;
/**
 * Strips normalization separators from extracted route parameters.
 * Used by both server and client code to clean up parameters after route matching.
 */
export declare function stripParameterSeparators(params: Record<string, any>): Record<string, any>;
