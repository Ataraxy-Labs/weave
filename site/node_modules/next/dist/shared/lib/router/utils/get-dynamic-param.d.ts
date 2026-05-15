import type { DynamicParam } from '../../../../server/app-render/app-render';
import type { LoaderTree } from '../../../../server/lib/app-dir-module';
import type { OpaqueFallbackRouteParams } from '../../../../server/request/fallback-params';
import type { Params } from '../../../../server/request/params';
import type { DynamicParamTypesShort } from '../../app-router-types';
export declare function interpolateParallelRouteParams(loaderTree: LoaderTree, params: Params, pagePath: string, fallbackRouteParams: OpaqueFallbackRouteParams | null): Params;
/**
 *
 * Shared logic on client and server for creating a dynamic param value.
 *
 * This code needs to be shared with the client so it can extract dynamic route
 * params from the URL without a server request.
 *
 * Because everything in this module is sent to the client, we should aim to
 * keep this code as simple as possible. The special case handling for catchall
 * and optional is, alas, unfortunate.
 */
export declare function getDynamicParam(interpolatedParams: Params, segmentKey: string, dynamicParamType: DynamicParamTypesShort, fallbackRouteParams: OpaqueFallbackRouteParams | null): DynamicParam;
/**
 * Regular expression pattern used to match route parameters.
 * Matches both single parameters and parameter groups.
 * Examples:
 *   - `[[...slug]]` matches parameter group with key 'slug', repeat: true, optional: true
 *   - `[...slug]` matches parameter group with key 'slug', repeat: true, optional: false
 *   - `[[foo]]` matches parameter with key 'foo', repeat: false, optional: true
 *   - `[bar]` matches parameter with key 'bar', repeat: false, optional: false
 */
export declare const PARAMETER_PATTERN: RegExp;
/**
 * Parses a given parameter from a route to a data structure that can be used
 * to generate the parametrized route.
 * Examples:
 *   - `[[...slug]]` -> `{ key: 'slug', repeat: true, optional: true }`
 *   - `[...slug]` -> `{ key: 'slug', repeat: true, optional: false }`
 *   - `[[foo]]` -> `{ key: 'foo', repeat: false, optional: true }`
 *   - `[bar]` -> `{ key: 'bar', repeat: false, optional: false }`
 *   - `fizz` -> `{ key: 'fizz', repeat: false, optional: false }`
 * @param param - The parameter to parse.
 * @returns The parsed parameter as a data structure.
 */
export declare function parseParameter(param: string): {
    key: string;
    repeat: boolean;
    optional: boolean;
};
/**
 * Parses a matched parameter from the PARAMETER_PATTERN regex to a data structure that can be used
 * to generate the parametrized route.
 * Examples:
 *   - `[...slug]` -> `{ key: 'slug', repeat: true, optional: true }`
 *   - `...slug` -> `{ key: 'slug', repeat: true, optional: false }`
 *   - `[foo]` -> `{ key: 'foo', repeat: false, optional: true }`
 *   - `bar` -> `{ key: 'bar', repeat: false, optional: false }`
 * @param param - The matched parameter to parse.
 * @returns The parsed parameter as a data structure.
 */
export declare function parseMatchedParameter(param: string): {
    key: string;
    repeat: boolean;
    optional: boolean;
};
