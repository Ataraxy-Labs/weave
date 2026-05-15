import { CachedRouteKind, IncrementalCacheKind } from '../../response-cache';
import path from '../../../shared/lib/isomorphic/path';
import { NEXT_CACHE_TAGS_HEADER, NEXT_DATA_SUFFIX, NEXT_META_SUFFIX, RSC_SEGMENT_SUFFIX, RSC_SEGMENTS_DIR_SUFFIX, RSC_SUFFIX } from '../../../lib/constants';
import { areTagsExpired, tagsManifest } from './tags-manifest.external';
import { MultiFileWriter } from '../../../lib/multi-file-writer';
import { getMemoryCache } from './memory-cache.external';
export default class FileSystemCache {
    static #_ = this.debug = !!process.env.NEXT_PRIVATE_DEBUG_CACHE;
    constructor(ctx){
        this.fs = ctx.fs;
        this.flushToDisk = ctx.flushToDisk;
        this.serverDistDir = ctx.serverDistDir;
        this.revalidatedTags = ctx.revalidatedTags;
        if (ctx.maxMemoryCacheSize) {
            if (!FileSystemCache.memoryCache) {
                if (FileSystemCache.debug) {
                    console.log('FileSystemCache: using memory store for fetch cache');
                }
                FileSystemCache.memoryCache = getMemoryCache(ctx.maxMemoryCacheSize);
            } else if (FileSystemCache.debug) {
                console.log('FileSystemCache: memory store already initialized');
            }
        } else if (FileSystemCache.debug) {
            console.log('FileSystemCache: not using memory store for fetch cache');
        }
    }
    resetRequestCache() {}
    async revalidateTag(tags, durations) {
        tags = typeof tags === 'string' ? [
            tags
        ] : tags;
        if (FileSystemCache.debug) {
            console.log('FileSystemCache: revalidateTag', tags, durations);
        }
        if (tags.length === 0) {
            return;
        }
        const now = Date.now();
        for (const tag of tags){
            const existingEntry = tagsManifest.get(tag) || {};
            if (durations) {
                // Use provided durations directly
                const updates = {
                    ...existingEntry
                };
                // mark as stale immediately
                updates.stale = now;
                if (durations.expire !== undefined) {
                    updates.expired = now + durations.expire * 1000 // Convert seconds to ms
                    ;
                }
                tagsManifest.set(tag, updates);
            } else {
                // Update expired field for immediate expiration (default behavior when no durations provided)
                tagsManifest.set(tag, {
                    ...existingEntry,
                    expired: now
                });
            }
        }
    }
    async get(...args) {
        var _FileSystemCache_memoryCache, _data_value, _data_value1, _data_value2, _data_value3;
        const [key, ctx] = args;
        const { kind } = ctx;
        let data = (_FileSystemCache_memoryCache = FileSystemCache.memoryCache) == null ? void 0 : _FileSystemCache_memoryCache.get(key);
        if (FileSystemCache.debug) {
            if (kind === IncrementalCacheKind.FETCH) {
                console.log('FileSystemCache: get', key, ctx.tags, kind, !!data);
            } else {
                console.log('FileSystemCache: get', key, kind, !!data);
            }
        }
        // let's check the disk for seed data
        if (!data && process.env.NEXT_RUNTIME !== 'edge') {
            try {
                if (kind === IncrementalCacheKind.APP_ROUTE) {
                    const filePath = this.getFilePath(`${key}.body`, IncrementalCacheKind.APP_ROUTE);
                    const fileData = await this.fs.readFile(filePath);
                    const { mtime } = await this.fs.stat(filePath);
                    const meta = JSON.parse(await this.fs.readFile(filePath.replace(/\.body$/, NEXT_META_SUFFIX), 'utf8'));
                    data = {
                        lastModified: mtime.getTime(),
                        value: {
                            kind: CachedRouteKind.APP_ROUTE,
                            body: fileData,
                            headers: meta.headers,
                            status: meta.status
                        }
                    };
                } else {
                    const filePath = this.getFilePath(kind === IncrementalCacheKind.FETCH ? key : `${key}.html`, kind);
                    const fileData = await this.fs.readFile(filePath, 'utf8');
                    const { mtime } = await this.fs.stat(filePath);
                    if (kind === IncrementalCacheKind.FETCH) {
                        var _data_value4;
                        const { tags, fetchIdx, fetchUrl } = ctx;
                        if (!this.flushToDisk) return null;
                        const lastModified = mtime.getTime();
                        const parsedData = JSON.parse(fileData);
                        data = {
                            lastModified,
                            value: parsedData
                        };
                        if (((_data_value4 = data.value) == null ? void 0 : _data_value4.kind) === CachedRouteKind.FETCH) {
                            var _data_value5;
                            const storedTags = (_data_value5 = data.value) == null ? void 0 : _data_value5.tags;
                            // update stored tags if a new one is being added
                            // TODO: remove this when we can send the tags
                            // via header on GET same as SET
                            if (!(tags == null ? void 0 : tags.every((tag)=>storedTags == null ? void 0 : storedTags.includes(tag)))) {
                                if (FileSystemCache.debug) {
                                    console.log('FileSystemCache: tags vs storedTags mismatch', tags, storedTags);
                                }
                                await this.set(key, data.value, {
                                    fetchCache: true,
                                    tags,
                                    fetchIdx,
                                    fetchUrl
                                });
                            }
                        }
                    } else if (kind === IncrementalCacheKind.APP_PAGE) {
                        // We try to load the metadata file, but if it fails, we don't
                        // error. We also don't load it if this is a fallback.
                        let meta;
                        try {
                            meta = JSON.parse(await this.fs.readFile(filePath.replace(/\.html$/, NEXT_META_SUFFIX), 'utf8'));
                        } catch  {}
                        let maybeSegmentData;
                        if (meta == null ? void 0 : meta.segmentPaths) {
                            // Collect all the segment data for this page.
                            // TODO: To optimize file system reads, we should consider creating
                            // separate cache entries for each segment, rather than storing them
                            // all on the page's entry. Though the behavior is
                            // identical regardless.
                            const segmentData = new Map();
                            maybeSegmentData = segmentData;
                            const segmentsDir = key + RSC_SEGMENTS_DIR_SUFFIX;
                            await Promise.all(meta.segmentPaths.map(async (segmentPath)=>{
                                const segmentDataFilePath = this.getFilePath(segmentsDir + segmentPath + RSC_SEGMENT_SUFFIX, IncrementalCacheKind.APP_PAGE);
                                try {
                                    segmentData.set(segmentPath, await this.fs.readFile(segmentDataFilePath));
                                } catch  {
                                // This shouldn't happen, but if for some reason we fail to
                                // load a segment from the filesystem, treat it the same as if
                                // the segment is dynamic and does not have a prefetch.
                                }
                            }));
                        }
                        let rscData;
                        if (!ctx.isFallback && !ctx.isRoutePPREnabled) {
                            rscData = await this.fs.readFile(this.getFilePath(`${key}${RSC_SUFFIX}`, IncrementalCacheKind.APP_PAGE));
                        }
                        data = {
                            lastModified: mtime.getTime(),
                            value: {
                                kind: CachedRouteKind.APP_PAGE,
                                html: fileData,
                                rscData,
                                postponed: meta == null ? void 0 : meta.postponed,
                                headers: meta == null ? void 0 : meta.headers,
                                status: meta == null ? void 0 : meta.status,
                                segmentData: maybeSegmentData
                            }
                        };
                    } else if (kind === IncrementalCacheKind.PAGES) {
                        let meta;
                        let pageData = {};
                        if (!ctx.isFallback) {
                            pageData = JSON.parse(await this.fs.readFile(this.getFilePath(`${key}${NEXT_DATA_SUFFIX}`, IncrementalCacheKind.PAGES), 'utf8'));
                        }
                        data = {
                            lastModified: mtime.getTime(),
                            value: {
                                kind: CachedRouteKind.PAGES,
                                html: fileData,
                                pageData,
                                headers: meta == null ? void 0 : meta.headers,
                                status: meta == null ? void 0 : meta.status
                            }
                        };
                    } else {
                        throw Object.defineProperty(new Error(`Invariant: Unexpected route kind ${kind} in file system cache.`), "__NEXT_ERROR_CODE", {
                            value: "E445",
                            enumerable: false,
                            configurable: true
                        });
                    }
                }
                if (data) {
                    var _FileSystemCache_memoryCache1;
                    (_FileSystemCache_memoryCache1 = FileSystemCache.memoryCache) == null ? void 0 : _FileSystemCache_memoryCache1.set(key, data);
                }
            } catch  {
                return null;
            }
        }
        if ((data == null ? void 0 : (_data_value = data.value) == null ? void 0 : _data_value.kind) === CachedRouteKind.APP_PAGE || (data == null ? void 0 : (_data_value1 = data.value) == null ? void 0 : _data_value1.kind) === CachedRouteKind.APP_ROUTE || (data == null ? void 0 : (_data_value2 = data.value) == null ? void 0 : _data_value2.kind) === CachedRouteKind.PAGES) {
            var _data_value_headers;
            const tagsHeader = (_data_value_headers = data.value.headers) == null ? void 0 : _data_value_headers[NEXT_CACHE_TAGS_HEADER];
            if (typeof tagsHeader === 'string') {
                const cacheTags = tagsHeader.split(',');
                // we trigger a blocking validation if an ISR page
                // had a tag revalidated, if we want to be a background
                // revalidation instead we return data.lastModified = -1
                if (cacheTags.length > 0 && areTagsExpired(cacheTags, data.lastModified)) {
                    if (FileSystemCache.debug) {
                        console.log('FileSystemCache: expired tags', cacheTags);
                    }
                    return null;
                }
            }
        } else if ((data == null ? void 0 : (_data_value3 = data.value) == null ? void 0 : _data_value3.kind) === CachedRouteKind.FETCH) {
            const combinedTags = ctx.kind === IncrementalCacheKind.FETCH ? [
                ...ctx.tags || [],
                ...ctx.softTags || []
            ] : [];
            // When revalidate tag is called we don't return stale data so it's
            // updated right away.
            if (combinedTags.some((tag)=>this.revalidatedTags.includes(tag))) {
                if (FileSystemCache.debug) {
                    console.log('FileSystemCache: was revalidated', combinedTags);
                }
                return null;
            }
            if (areTagsExpired(combinedTags, data.lastModified)) {
                if (FileSystemCache.debug) {
                    console.log('FileSystemCache: expired tags', combinedTags);
                }
                return null;
            }
        }
        return data ?? null;
    }
    async set(key, data, ctx) {
        var _FileSystemCache_memoryCache;
        (_FileSystemCache_memoryCache = FileSystemCache.memoryCache) == null ? void 0 : _FileSystemCache_memoryCache.set(key, {
            value: data,
            lastModified: Date.now()
        });
        if (FileSystemCache.debug) {
            console.log('FileSystemCache: set', key);
        }
        if (!this.flushToDisk || !data) return;
        // Create a new writer that will prepare to write all the files to disk
        // after their containing directory is created.
        const writer = new MultiFileWriter(this.fs);
        if (data.kind === CachedRouteKind.APP_ROUTE) {
            const filePath = this.getFilePath(`${key}.body`, IncrementalCacheKind.APP_ROUTE);
            writer.append(filePath, data.body);
            const meta = {
                headers: data.headers,
                status: data.status,
                postponed: undefined,
                segmentPaths: undefined
            };
            writer.append(filePath.replace(/\.body$/, NEXT_META_SUFFIX), JSON.stringify(meta, null, 2));
        } else if (data.kind === CachedRouteKind.PAGES || data.kind === CachedRouteKind.APP_PAGE) {
            const isAppPath = data.kind === CachedRouteKind.APP_PAGE;
            const htmlPath = this.getFilePath(`${key}.html`, isAppPath ? IncrementalCacheKind.APP_PAGE : IncrementalCacheKind.PAGES);
            writer.append(htmlPath, data.html);
            // Fallbacks don't generate a data file.
            if (!ctx.fetchCache && !ctx.isFallback && !ctx.isRoutePPREnabled) {
                writer.append(this.getFilePath(`${key}${isAppPath ? RSC_SUFFIX : NEXT_DATA_SUFFIX}`, isAppPath ? IncrementalCacheKind.APP_PAGE : IncrementalCacheKind.PAGES), isAppPath ? data.rscData : JSON.stringify(data.pageData));
            }
            if ((data == null ? void 0 : data.kind) === CachedRouteKind.APP_PAGE) {
                let segmentPaths;
                if (data.segmentData) {
                    segmentPaths = [];
                    const segmentsDir = htmlPath.replace(/\.html$/, RSC_SEGMENTS_DIR_SUFFIX);
                    for (const [segmentPath, buffer] of data.segmentData){
                        segmentPaths.push(segmentPath);
                        const segmentDataFilePath = segmentsDir + segmentPath + RSC_SEGMENT_SUFFIX;
                        writer.append(segmentDataFilePath, buffer);
                    }
                }
                const meta = {
                    headers: data.headers,
                    status: data.status,
                    postponed: data.postponed,
                    segmentPaths
                };
                writer.append(htmlPath.replace(/\.html$/, NEXT_META_SUFFIX), JSON.stringify(meta));
            }
        } else if (data.kind === CachedRouteKind.FETCH) {
            const filePath = this.getFilePath(key, IncrementalCacheKind.FETCH);
            writer.append(filePath, JSON.stringify({
                ...data,
                tags: ctx.fetchCache ? ctx.tags : []
            }));
        }
        // Wait for all FS operations to complete.
        await writer.wait();
    }
    getFilePath(pathname, kind) {
        switch(kind){
            case IncrementalCacheKind.FETCH:
                // we store in .next/cache/fetch-cache so it can be persisted
                // across deploys
                return path.join(this.serverDistDir, '..', 'cache', 'fetch-cache', pathname);
            case IncrementalCacheKind.PAGES:
                return path.join(this.serverDistDir, 'pages', pathname);
            case IncrementalCacheKind.IMAGE:
            case IncrementalCacheKind.APP_PAGE:
            case IncrementalCacheKind.APP_ROUTE:
                return path.join(this.serverDistDir, 'app', pathname);
            default:
                throw Object.defineProperty(new Error(`Unexpected file path kind: ${kind}`), "__NEXT_ERROR_CODE", {
                    value: "E479",
                    enumerable: false,
                    configurable: true
                });
        }
    }
}

//# sourceMappingURL=file-system-cache.js.map