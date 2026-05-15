export declare function isStableBuild(): boolean;
export declare class CanaryOnlyConfigError extends Error {
    constructor(arg: {
        feature: string;
    } | string);
}
