function decodeHex(hexStr) {
    if (hexStr.trim() === '') {
        throw Object.defineProperty(new Error("can't decode empty hex"), "__NEXT_ERROR_CODE", {
            value: "E19",
            enumerable: false,
            configurable: true
        });
    }
    const num = parseInt(hexStr, 16);
    if (isNaN(num)) {
        throw Object.defineProperty(new Error(`invalid hex: \`${hexStr}\``), "__NEXT_ERROR_CODE", {
            value: "E293",
            enumerable: false,
            configurable: true
        });
    }
    return String.fromCodePoint(num);
}
const DECODE_REGEX = /^__TURBOPACK__([a-zA-Z0-9_$]+)__$/;
export function decodeMagicIdentifier(identifier) {
    const matches = identifier.match(DECODE_REGEX);
    if (!matches) {
        return identifier;
    }
    const inner = matches[1];
    let output = '';
    let mode = 0;
    let buffer = '';
    for(let i = 0; i < inner.length; i++){
        const char = inner[i];
        if (mode === 0) {
            if (char === '_') {
                mode = 1;
            } else if (char === '$') {
                mode = 2;
            } else {
                output += char;
            }
        } else if (mode === 1) {
            if (char === '_') {
                output += ' ';
                mode = 0;
            } else if (char === '$') {
                output += '_';
                mode = 2;
            } else {
                output += char;
                mode = 0;
            }
        } else if (mode === 2) {
            if (buffer.length === 2) {
                output += decodeHex(buffer);
                buffer = '';
            }
            if (char === '_') {
                if (buffer !== '') {
                    throw Object.defineProperty(new Error(`invalid hex: \`${buffer}\``), "__NEXT_ERROR_CODE", {
                        value: "E293",
                        enumerable: false,
                        configurable: true
                    });
                }
                mode = 3;
            } else if (char === '$') {
                if (buffer !== '') {
                    throw Object.defineProperty(new Error(`invalid hex: \`${buffer}\``), "__NEXT_ERROR_CODE", {
                        value: "E293",
                        enumerable: false,
                        configurable: true
                    });
                }
                mode = 0;
            } else {
                buffer += char;
            }
        } else if (mode === 3) {
            if (char === '_') {
                throw Object.defineProperty(new Error(`invalid hex: \`${buffer + char}\``), "__NEXT_ERROR_CODE", {
                    value: "E244",
                    enumerable: false,
                    configurable: true
                });
            } else if (char === '$') {
                output += decodeHex(buffer);
                buffer = '';
                mode = 0;
            } else {
                buffer += char;
            }
        }
    }
    return output;
}
export const MAGIC_IDENTIFIER_REGEX = /__TURBOPACK__[a-zA-Z0-9_$]+__/g;
/**
 * Cleans up module IDs by removing implementation details.
 * - Replaces [project] with .
 * - Removes content in brackets [], parentheses (), and angle brackets <>
 */ export function deobfuscateModuleId(moduleId) {
    return moduleId// Replace [project] with .
    .replace(/\[project\]/g, '.')// Remove content in square brackets (e.g. [app-rsc])
    .replace(/\s\[([^\]]*)\]/g, '')// Remove content in parentheses (e.g. (ecmascript))
    .replace(/\s\(([^)]*)\)/g, '')// Remove content in angle brackets (e.g. <locals>)
    .replace(/\s<([^>]*)>/g, '')// Clean up any extra whitespace
    .trim();
}
/**
 * Removes the free call wrapper pattern (0, expr) from expressions.
 * This is a JavaScript pattern to call a function without binding 'this',
 * but it's noise for developers reading error messages.
 */ export function removeFreeCallWrapper(text) {
    // Match (0, <ident>.<ident>) patterns anywhere in the text the beginning
    // Use Unicode property escapes (\p{ID_Start}, \p{ID_Continue}) for full JS identifier support
    // Requires the 'u' (unicode) flag in the regex
    return text.replace(/\(0\s*,\s*(__TURBOPACK__[a-zA-Z0-9_$]+__\.[\p{ID_Start}_$][\p{ID_Continue}$]*)\)/u, '$1');
}
/**
 * Deobfuscates text and returns an array of discriminated parts.
 * Each part is a tuple of [type, string] where type is either 'raw' (unchanged text)
 * or 'deobfuscated' (a magic identifier that was decoded).
 *
 * This is useful when you need to process or display deobfuscated and raw text differently.
 */ export function deobfuscateTextParts(text) {
    // First, remove free call wrappers
    const withoutFreeCall = removeFreeCallWrapper(text);
    const parts = [];
    let lastIndex = 0;
    // Create a new regex instance for global matching
    const regex = new RegExp(MAGIC_IDENTIFIER_REGEX.source, 'g');
    for(let match = regex.exec(withoutFreeCall); match !== null; match = regex.exec(withoutFreeCall)){
        const matchStart = match.index;
        const matchEnd = regex.lastIndex;
        const ident = match[0];
        // Add raw text before this match (if any)
        if (matchStart > lastIndex) {
            const rawText = withoutFreeCall.substring(lastIndex, matchStart);
            parts.push([
                'raw',
                rawText
            ]);
        }
        // Process and add the deobfuscated part
        try {
            const decoded = decodeMagicIdentifier(ident);
            // If it was a magic identifier, clean up the module ID
            if (decoded !== ident) {
                // Check if this is an "imported module" reference
                const importedModuleMatch = decoded.match(/^imported module (.+)$/);
                if (importedModuleMatch) {
                    // Clean the entire module path (which includes [app-rsc], etc.)
                    const modulePathWithMetadata = importedModuleMatch[1];
                    const cleaned = deobfuscateModuleId(modulePathWithMetadata);
                    parts.push([
                        'deobfuscated',
                        `{imported module ${cleaned}}`
                    ]);
                } else {
                    const cleaned = deobfuscateModuleId(decoded);
                    parts.push([
                        'deobfuscated',
                        `{${cleaned}}`
                    ]);
                }
            } else {
                // Not actually a magic identifier, treat as raw
                parts.push([
                    'raw',
                    ident
                ]);
            }
        } catch (e) {
            parts.push([
                'deobfuscated',
                `{${ident} (decoding failed: ${e})}`
            ]);
        }
        lastIndex = matchEnd;
    }
    // Add any remaining raw text after the last match
    if (lastIndex < withoutFreeCall.length) {
        const rawText = withoutFreeCall.substring(lastIndex);
        parts.push([
            'raw',
            rawText
        ]);
    }
    return parts;
}
/**
 * Deobfuscates text by:
 * 1. Decoding magic identifiers
 * 2. Cleaning up module IDs
 * 3. Removing free call wrappers
 */ export function deobfuscateText(text) {
    const parts = deobfuscateTextParts(text);
    return parts.map((part)=>part[1]).join('');
}

//# sourceMappingURL=magic-identifier.js.map