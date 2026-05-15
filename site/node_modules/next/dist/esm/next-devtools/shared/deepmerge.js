export function deepMerge(target, source) {
    if (!source || typeof source !== 'object' || Array.isArray(source)) {
        return source;
    }
    if (!target || typeof target !== 'object' || Array.isArray(target)) {
        return source;
    }
    const result = {
        ...target
    };
    for(const key in source){
        const sourceValue = source[key];
        const targetValue = target[key];
        if (sourceValue !== undefined) {
            if (sourceValue && typeof sourceValue === 'object' && !Array.isArray(sourceValue) && targetValue && typeof targetValue === 'object' && !Array.isArray(targetValue)) {
                result[key] = deepMerge(targetValue, sourceValue);
            } else {
                result[key] = sourceValue;
            }
        }
    }
    return result;
}

//# sourceMappingURL=deepmerge.js.map