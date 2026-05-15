import { getClientReferenceManifest } from './manifests-singleton';
/**
 * Get external stylesheet link hrefs based on server CSS manifest.
 */ export function getLinkAndScriptTags(filePath, injectedCSS, injectedScripts, collectNewImports) {
    const filePathWithoutExt = filePath.replace(/\.[^.]+$/, '');
    const cssChunks = new Set();
    const jsChunks = new Set();
    const { entryCSSFiles, entryJSFiles } = getClientReferenceManifest();
    const cssFiles = entryCSSFiles[filePathWithoutExt];
    const jsFiles = entryJSFiles == null ? void 0 : entryJSFiles[filePathWithoutExt];
    if (cssFiles) {
        for (const css of cssFiles){
            if (!injectedCSS.has(css.path)) {
                if (collectNewImports) {
                    injectedCSS.add(css.path);
                }
                cssChunks.add(css);
            }
        }
    }
    if (jsFiles) {
        for (const file of jsFiles){
            if (!injectedScripts.has(file)) {
                if (collectNewImports) {
                    injectedScripts.add(file);
                }
                jsChunks.add(file);
            }
        }
    }
    return {
        styles: [
            ...cssChunks
        ],
        scripts: [
            ...jsChunks
        ]
    };
}

//# sourceMappingURL=get-css-inlined-link-tags.js.map