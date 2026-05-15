"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
0 && (module.exports = {
    BOUNDARY_PREFIX: null,
    BOUNDARY_SUFFIX: null,
    BUILTIN_PREFIX: null,
    getBoundaryOriginFileType: null,
    getConventionPathByType: null,
    isBoundaryFile: null,
    isBuiltinBoundaryFile: null,
    isNextjsBuiltinFilePath: null,
    normalizeBoundaryFilename: null,
    normalizeConventionFilePath: null
});
function _export(target, all) {
    for(var name in all)Object.defineProperty(target, name, {
        enumerable: true,
        get: all[name]
    });
}
_export(exports, {
    BOUNDARY_PREFIX: function() {
        return BOUNDARY_PREFIX;
    },
    BOUNDARY_SUFFIX: function() {
        return BOUNDARY_SUFFIX;
    },
    BUILTIN_PREFIX: function() {
        return BUILTIN_PREFIX;
    },
    getBoundaryOriginFileType: function() {
        return getBoundaryOriginFileType;
    },
    getConventionPathByType: function() {
        return getConventionPathByType;
    },
    isBoundaryFile: function() {
        return isBoundaryFile;
    },
    isBuiltinBoundaryFile: function() {
        return isBuiltinBoundaryFile;
    },
    isNextjsBuiltinFilePath: function() {
        return isNextjsBuiltinFilePath;
    },
    normalizeBoundaryFilename: function() {
        return normalizeBoundaryFilename;
    },
    normalizeConventionFilePath: function() {
        return normalizeConventionFilePath;
    }
});
const BUILTIN_PREFIX = '__next_builtin__';
const nextInternalPrefixRegex = /^(.*[\\/])?next[\\/]dist[\\/]client[\\/]components[\\/]builtin[\\/]/;
function normalizeConventionFilePath(projectDir, conventionPath) {
    // Turbopack project path is formed as: "<project root>/<cwd>".
    // When project root is not the working directory, we can extract the relative project root path.
    // This is mostly used for running Next.js inside a monorepo.
    const cwd = process.env.NEXT_RUNTIME === 'edge' ? '' : process.cwd();
    const relativeProjectRoot = projectDir.replace(cwd, '');
    let relativePath = (conventionPath || '')// remove turbopack [project] prefix
    .replace(/^\[project\]/, '')// remove turbopack relative project path, everything after [project] and before the working directory.
    .replace(relativeProjectRoot, '')// remove the project root from the path
    .replace(projectDir, '')// remove cwd prefix
    .replace(cwd, '')// remove /(src/)?app/ dir prefix
    .replace(/^([\\/])*(src[\\/])?app[\\/]/, '');
    // If it's internal file only keep the filename, strip nextjs internal prefix
    if (nextInternalPrefixRegex.test(relativePath)) {
        relativePath = relativePath.replace(nextInternalPrefixRegex, '');
        // Add a special prefix to let segment explorer know it's a built-in component
        relativePath = `${BUILTIN_PREFIX}${relativePath}`;
    }
    return relativePath.replace(/\\/g, '/');
}
const isNextjsBuiltinFilePath = (filePath)=>{
    return nextInternalPrefixRegex.test(filePath);
};
const BOUNDARY_SUFFIX = '@boundary';
function normalizeBoundaryFilename(filename) {
    return filename.replace(new RegExp(`^${BUILTIN_PREFIX}`), '').replace(new RegExp(`${BOUNDARY_SUFFIX}$`), '');
}
const BOUNDARY_PREFIX = 'boundary:';
function isBoundaryFile(fileType) {
    return fileType.startsWith(BOUNDARY_PREFIX);
}
function isBuiltinBoundaryFile(fileType) {
    return fileType.startsWith(BUILTIN_PREFIX);
}
function getBoundaryOriginFileType(fileType) {
    return fileType.replace(BOUNDARY_PREFIX, '');
}
function getConventionPathByType(tree, dir, conventionType) {
    const modules = tree[2];
    const conventionPath = modules[conventionType] ? modules[conventionType][1] : undefined;
    if (conventionPath) {
        return normalizeConventionFilePath(dir, conventionPath);
    }
    return undefined;
}

//# sourceMappingURL=segment-explorer-path.js.map