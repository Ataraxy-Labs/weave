import { type GeneratorResult } from 'next/dist/compiled/babel/generator';
import type { Span } from '../../../trace';
import type { NextJsLoaderContext } from './types';
import type { SourceMap } from './util';
export default function transform(ctx: NextJsLoaderContext, source: string, inputSourceMap: SourceMap | null | undefined, loaderOptions: any, filename: string, target: string, parentSpan: Span): Promise<GeneratorResult>;
