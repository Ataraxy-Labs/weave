export declare function decodeMagicIdentifier(identifier: string): string;
export declare const MAGIC_IDENTIFIER_REGEX: RegExp;
/**
 * Cleans up module IDs by removing implementation details.
 * - Replaces [project] with .
 * - Removes content in brackets [], parentheses (), and angle brackets <>
 */
export declare function deobfuscateModuleId(moduleId: string): string;
/**
 * Removes the free call wrapper pattern (0, expr) from expressions.
 * This is a JavaScript pattern to call a function without binding 'this',
 * but it's noise for developers reading error messages.
 */
export declare function removeFreeCallWrapper(text: string): string;
export type TextPartType = 'raw' | 'deobfuscated';
/**
 * Deobfuscates text and returns an array of discriminated parts.
 * Each part is a tuple of [type, string] where type is either 'raw' (unchanged text)
 * or 'deobfuscated' (a magic identifier that was decoded).
 *
 * This is useful when you need to process or display deobfuscated and raw text differently.
 */
export declare function deobfuscateTextParts(text: string): Array<[TextPartType, string]>;
/**
 * Deobfuscates text by:
 * 1. Decoding magic identifiers
 * 2. Cleaning up module IDs
 * 3. Removing free call wrappers
 */
export declare function deobfuscateText(text: string): string;
