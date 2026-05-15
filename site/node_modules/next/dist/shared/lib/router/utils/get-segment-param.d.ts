import type { DynamicParamTypes } from '../../app-router-types';
export type SegmentParam = {
    paramName: string;
    paramType: DynamicParamTypes;
};
/**
 * Parse dynamic route segment to type of parameter
 */
export declare function getSegmentParam(segment: string): SegmentParam | null;
export declare function isCatchAll(type: DynamicParamTypes): type is 'catchall' | 'catchall-intercepted-(..)(..)' | 'catchall-intercepted-(.)' | 'catchall-intercepted-(..)' | 'catchall-intercepted-(...)' | 'optional-catchall';
export declare function getParamProperties(paramType: DynamicParamTypes): {
    repeat: boolean;
    optional: boolean;
};
