import type { CssResource } from '../../build/webpack/plugins/flight-manifest-plugin';
/**
 * Get external stylesheet link hrefs based on server CSS manifest.
 */
export declare function getLinkAndScriptTags(filePath: string, injectedCSS: Set<string>, injectedScripts: Set<string>, collectNewImports?: boolean): {
    styles: CssResource[];
    scripts: string[];
};
