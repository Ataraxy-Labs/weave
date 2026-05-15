/**
 * A route that can be sorted by specificity.
 */ "use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
0 && (module.exports = {
    compareRouteSegments: null,
    getSegmentSpecificity: null,
    sortPageObjects: null,
    sortPages: null,
    sortSortableRouteObjects: null,
    sortSortableRoutes: null
});
function _export(target, all) {
    for(var name in all)Object.defineProperty(target, name, {
        enumerable: true,
        get: all[name]
    });
}
_export(exports, {
    compareRouteSegments: function() {
        return compareRouteSegments;
    },
    getSegmentSpecificity: function() {
        return getSegmentSpecificity;
    },
    sortPageObjects: function() {
        return sortPageObjects;
    },
    sortPages: function() {
        return sortPages;
    },
    sortSortableRouteObjects: function() {
        return sortSortableRouteObjects;
    },
    sortSortableRoutes: function() {
        return sortSortableRoutes;
    }
});
function getSegmentSpecificity(segment) {
    // Static segments are most specific - they match exactly one path
    if (!segment.includes('[')) {
        return 0;
    }
    // Optional catch-all [[...param]] is least specific - matches zero or more segments
    if (segment.startsWith('[[...') && segment.endsWith(']]')) {
        return 3;
    }
    // Catch-all [...param] is less specific - matches one or more segments
    if (segment.startsWith('[...') && segment.endsWith(']')) {
        return 2;
    }
    // Regular dynamic [param] is more specific than catch-all - matches exactly one segment
    if (segment.startsWith('[') && segment.endsWith(']')) {
        return 1;
    }
    // Default to static (fallback case)
    return 0;
}
function compareRouteSegments(pathA, pathB) {
    // Split paths into segments, removing empty strings from leading/trailing slashes
    const segmentsA = pathA.split('/').filter(Boolean);
    const segmentsB = pathB.split('/').filter(Boolean);
    // Compare segment by segment up to the length of the longer path
    const maxLength = Math.max(segmentsA.length, segmentsB.length);
    for(let i = 0; i < maxLength; i++){
        const segA = segmentsA[i] || '';
        const segB = segmentsB[i] || '';
        // Handle length differences: shorter routes are MORE specific
        // Example: "/api" is more specific than "/api/users"
        if (!segA && segB) return -1 // pathA is shorter, so more specific
        ;
        if (segA && !segB) return 1 // pathB is shorter, so more specific
        ;
        if (!segA && !segB) return 0 // Both paths ended, they're equal
        ;
        // Compare segment specificity using our specificity scoring
        const specificityA = getSegmentSpecificity(segA);
        const specificityB = getSegmentSpecificity(segB);
        // Lower specificity number = more specific route
        // Example: "api" (0) vs "[slug]" (1) - "api" wins
        if (specificityA !== specificityB) {
            return specificityA - specificityB;
        }
        // If segments have same specificity, compare lexicographically for determinism
        // Example: "[id]" vs "[slug]" - "[id]" comes first alphabetically
        if (segA !== segB) {
            return segA.localeCompare(segB);
        }
    // Segments are identical, continue to next segment
    }
    // All segments compared equally
    return 0;
}
/**
 * Compares two complete routes for sorting purposes.
 *
 * Routes are compared with a two-tier priority system:
 * 1. Primary: Compare by source path specificity
 * 2. Secondary: If sources are equal, compare by page path specificity
 *
 * This ensures that routes are primarily organized by their source patterns,
 * with page-specific variations grouped together.
 *
 * @param a - First route to compare
 * @param b - Second route to compare
 * @returns Negative if route a should come first, positive if route b should come first, 0 if equal
 */ function compareSortableRoutes(a, b) {
    // First compare by source specificity - this is the primary sorting criterion
    // Source represents the original route pattern and takes precedence
    const sourceResult = compareRouteSegments(a.sourcePage, b.sourcePage);
    if (sourceResult !== 0) return sourceResult;
    // If sources are identical, compare by page specificity as a tiebreaker
    // Page represents the final rendered route and provides secondary ordering
    return compareRouteSegments(a.page, b.page);
}
function sortSortableRoutes(routes) {
    // Because sort is always in-place, we need to create a shallow copy to avoid
    // mutating the input array.
    return [
        ...routes
    ].sort(compareSortableRoutes);
}
function sortPages(pages) {
    // Because sort is always in-place, we need to create a shallow copy to avoid
    // mutating the input array.
    return [
        ...pages
    ].sort(compareRouteSegments);
}
function sortSortableRouteObjects(objects, getter) {
    // Create a SortableRoute for each object.
    const routes = [];
    for (const object of objects){
        const route = getter(object);
        routes.push({
            ...route,
            object
        });
    }
    // In-place sort the SortableRoutes.
    routes.sort(compareSortableRoutes);
    // Map the sorted SortableRoutes back to the original objects.
    return routes.map(({ object })=>object);
}
function sortPageObjects(objects, getter) {
    const indexes = {};
    const pages = new Set();
    for(let i = 0; i < objects.length; i++){
        const object = objects[i];
        const page = getter(object);
        indexes[page]?.push(i) || (indexes[page] = [
            i
        ]);
        pages.add(page);
    }
    // Sort the unique pages.
    const sortedPages = Array.from(pages).sort(compareRouteSegments);
    // Map the sorted pages back to the original objects.
    return sortedPages.reduce((sortedObjects, page)=>{
        // Add all objects for this page to the sorted array.
        for (const i of indexes[page]){
            sortedObjects.push(objects[i]);
        }
        // Return the sorted array.
        return sortedObjects;
    }, []);
}

//# sourceMappingURL=sortable-routes.js.map