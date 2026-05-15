export const DEFAULT_MAX_POSTPONED_STATE_SIZE = '100 MB';
function parseSizeLimit(size) {
    const bytes = require('next/dist/compiled/bytes').parse(size);
    if (bytes === null || isNaN(bytes) || bytes < 1) {
        return undefined;
    }
    return bytes;
}
/**
 * Parses the maxPostponedStateSize config value, using the default if not provided.
 */ export function parseMaxPostponedStateSize(size) {
    return parseSizeLimit(size ?? DEFAULT_MAX_POSTPONED_STATE_SIZE);
}

//# sourceMappingURL=size-limit.js.map