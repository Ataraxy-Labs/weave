export declare function getOwnerStack(error: Error): string | null | undefined;
export declare function setOwnerStack(error: Error, stack: string | null): void;
export declare function coerceError(value: unknown): Error;
export declare function setOwnerStackIfAvailable(error: Error): void;
export declare function decorateDevError(thrownValue: unknown): Error;
