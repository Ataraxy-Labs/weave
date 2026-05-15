import { hrtimeBigIntDurationToString } from '../../build/duration-to-string';
import { blue, bold, gray, green, red, white, yellow, dim } from '../../lib/picocolors';
import { stripNextRscUnionQuery } from '../../lib/url';
import { getRequestMeta } from '../request-meta';
/**
 * Returns true if the incoming request should be ignored for logging.
 */ export function ignoreLoggingIncomingRequests(request, loggingConfig) {
    var _loggingConfig_incomingRequests;
    // If it's boolean use the boolean value
    if (typeof (loggingConfig == null ? void 0 : loggingConfig.incomingRequests) === 'boolean') {
        return !loggingConfig.incomingRequests;
    }
    // Any of the value on the chain is falsy, will not ignore the request.
    const ignore = loggingConfig == null ? void 0 : (_loggingConfig_incomingRequests = loggingConfig.incomingRequests) == null ? void 0 : _loggingConfig_incomingRequests.ignore;
    // If ignore is not set, don't ignore anything
    if (!ignore) {
        return false;
    }
    // If array of RegExp, ignore if any pattern matches
    return ignore.some((pattern)=>pattern.test(request.url));
}
export function logRequests(request, response, loggingConfig, requestStartTime, requestEndTime, devRequestTimingMiddlewareStart, devRequestTimingMiddlewareEnd, devRequestTimingInternalsEnd, devGenerateStaticParamsDuration) {
    if (!ignoreLoggingIncomingRequests(request, loggingConfig)) {
        logIncomingRequests(request, requestStartTime, requestEndTime, response.statusCode, devRequestTimingMiddlewareStart, devRequestTimingMiddlewareEnd, devRequestTimingInternalsEnd, devGenerateStaticParamsDuration);
    }
    if (request.fetchMetrics) {
        for (const fetchMetric of request.fetchMetrics){
            logFetchMetric(fetchMetric, loggingConfig);
        }
    }
}
function logIncomingRequests(request, requestStartTime, requestEndTime, statusCode, devRequestTimingMiddlewareStart, devRequestTimingMiddlewareEnd, devRequestTimingInternalsEnd, devGenerateStaticParamsDuration) {
    const isRSC = getRequestMeta(request, 'isRSCRequest');
    const url = isRSC ? stripNextRscUnionQuery(request.url) : request.url;
    const statusCodeColor = statusCode < 200 ? white : statusCode < 300 ? green : statusCode < 400 ? blue : statusCode < 500 ? yellow : red;
    const coloredStatus = statusCodeColor(statusCode.toString());
    const totalRequestTime = requestEndTime - requestStartTime;
    const times = [];
    let middlewareTime;
    if (devRequestTimingMiddlewareStart && devRequestTimingMiddlewareEnd) {
        middlewareTime = devRequestTimingMiddlewareEnd - devRequestTimingMiddlewareStart;
        times.push([
            'proxy.ts',
            middlewareTime
        ]);
    }
    if (devRequestTimingInternalsEnd) {
        let frameworkTime = devRequestTimingInternalsEnd - requestStartTime;
        /* Middleware runs during the internals so we have to subtract it from the framework time */ if (middlewareTime) {
            frameworkTime -= middlewareTime;
        }
        // Insert as the first item to be rendered in the list
        times.unshift([
            'compile',
            frameworkTime
        ]);
        // Insert after compile, before render based on the execution order.
        if (devGenerateStaticParamsDuration) {
            // Pages Router getStaticPaths are technically "generate params" as well.
            times.push([
                'generate-params',
                devGenerateStaticParamsDuration
            ]);
        }
        times.push([
            'render',
            requestEndTime - devRequestTimingInternalsEnd
        ]);
    }
    return writeLine(`${request.method} ${url} ${coloredStatus} in ${hrtimeBigIntDurationToString(totalRequestTime)}${times.length > 0 ? dim(` (${times.map(([label, time])=>`${label}: ${hrtimeBigIntDurationToString(time)}`).join(', ')})`) : ''}`);
}
function logFetchMetric(fetchMetric, loggingConfig) {
    var _loggingConfig_fetches;
    let { cacheReason, cacheStatus, cacheWarning, end, method, start, status, url } = fetchMetric;
    if (cacheStatus === 'hmr' && !(loggingConfig == null ? void 0 : (_loggingConfig_fetches = loggingConfig.fetches) == null ? void 0 : _loggingConfig_fetches.hmrRefreshes)) {
        // Cache hits during HMR refreshes are intentionally not logged, unless
        // explicitly enabled in the logging config.
        return;
    }
    if (loggingConfig == null ? void 0 : loggingConfig.fetches) {
        if (url.length > 48 && !loggingConfig.fetches.fullUrl) {
            url = truncateUrl(url);
        }
        writeLine(white(`${method} ${url} ${status} in ${Math.round(end - start)}ms ${formatCacheStatus(cacheStatus)}`), 1);
        if (cacheStatus === 'skip' || cacheStatus === 'miss') {
            writeLine(gray(`Cache ${cacheStatus === 'skip' ? 'skipped' : 'missed'} reason: (${white(cacheReason)})`), 2);
        }
    } else if (cacheWarning) {
        // When logging for fetches is not enabled, we still want to print any
        // associated warnings, so we print the request first to provide context.
        writeLine(white(`${method} ${url}`), 1);
    }
    if (cacheWarning) {
        writeLine(`${yellow(bold('⚠'))} ${white(cacheWarning)}`, 2);
    }
}
function writeLine(text, indentationLevel = 0) {
    process.stdout.write(` ${'│ '.repeat(indentationLevel)}${text}\n`);
}
function truncate(text, maxLength) {
    return maxLength !== undefined && text.length > maxLength ? text.substring(0, maxLength) + '..' : text;
}
function truncateUrl(url) {
    const { protocol, host, pathname, search } = new URL(url);
    return protocol + '//' + truncate(host, 16) + truncate(pathname, 24) + truncate(search, 16);
}
function formatCacheStatus(cacheStatus) {
    switch(cacheStatus){
        case 'hmr':
            return green('(HMR cache)');
        case 'hit':
            return green('(cache hit)');
        case 'miss':
        case 'skip':
            return yellow(`(cache ${cacheStatus})`);
        default:
            return cacheStatus;
    }
}

//# sourceMappingURL=log-requests.js.map