/**
 * Client-safe utilities for route matching that don't import server-side
 * utilities to avoid bundling issues with Turbopack
 */
import type { Key, TokensToRegexpOptions, ParseOptions, TokensToFunctionOptions } from 'next/dist/compiled/path-to-regexp';
/**
 * Client-safe wrapper around pathToRegexp that handles path-to-regexp 6.3.0+ validation errors.
 * This includes both "Can not repeat without prefix/suffix" and "Must have text between parameters" errors.
 */
export declare function safePathToRegexp(route: string | RegExp | Array<string | RegExp>, keys?: Key[], options?: TokensToRegexpOptions & ParseOptions): RegExp;
/**
 * Client-safe wrapper around compile that handles path-to-regexp 6.3.0+ validation errors.
 * No server-side error reporting to avoid bundling issues.
 * When normalization is applied, the returned compiler function automatically strips
 * the internal separator from the output URL.
 */
export declare function safeCompile(route: string, options?: TokensToFunctionOptions & ParseOptions): import("path-to-regexp").PathFunction<object>;
/**
 * Client-safe wrapper around regexpToFunction that automatically cleans parameters.
 */
export declare function safeRegexpToFunction<T extends Record<string, any> = Record<string, any>>(regexp: RegExp, keys?: Key[]): (pathname: string) => {
    params: T;
} | false;
/**
 * Safe wrapper for route matcher functions that automatically cleans parameters.
 * This is client-safe and doesn't import path-to-regexp.
 */
export declare function safeRouteMatcher<T extends Record<string, any>>(matcherFn: (pathname: string) => false | T): (pathname: string) => false | T;
