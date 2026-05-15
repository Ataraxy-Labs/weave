import React from 'react';
import type { GetDynamicParamFromSegment } from '../../server/app-render/app-render';
import type { LoaderTree } from '../../server/lib/app-dir-module';
import type { SearchParams } from '../../server/request/search-params';
import { type MetadataErrorType } from './resolve-metadata';
import type { MetadataContext } from './types/resolvers';
import type { WorkStore } from '../../server/app-render/work-async-storage.external';
export declare function createMetadataComponents({ tree, pathname, parsedQuery, metadataContext, getDynamicParamFromSegment, errorType, workStore, serveStreamingMetadata, }: {
    tree: LoaderTree;
    pathname: string;
    parsedQuery: SearchParams;
    metadataContext: MetadataContext;
    getDynamicParamFromSegment: GetDynamicParamFromSegment;
    errorType?: MetadataErrorType | 'redirect';
    workStore: WorkStore;
    serveStreamingMetadata: boolean;
}): {
    Viewport: React.ComponentType;
    Metadata: React.ComponentType;
    MetadataOutlet: React.ComponentType;
};
