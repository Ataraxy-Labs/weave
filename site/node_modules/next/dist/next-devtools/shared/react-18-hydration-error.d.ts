export declare function isHydrationError(error: unknown): boolean;
export declare function isHydrationWarning(message: unknown): message is string;
type NullableText = string | null | undefined;
export declare const getHydrationWarningType: (message: NullableText) => "tag" | "text" | "text-in-tag";
export {};
