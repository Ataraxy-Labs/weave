import type { RenderOpts } from './render';
declare function postProcessHTML(content: string, renderOpts: Pick<RenderOpts, 'optimizeCss' | 'distDir' | 'assetPrefix'>): Promise<string>;
export { postProcessHTML };
