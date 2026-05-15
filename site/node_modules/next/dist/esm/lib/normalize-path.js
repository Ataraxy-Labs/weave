import path from 'path';
export function normalizePath(file) {
    return path.sep === '\\' ? file.replace(/\\/g, '/') : file;
}

//# sourceMappingURL=normalize-path.js.map