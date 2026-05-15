import { getDynamicParam } from '../../shared/lib/router/utils/get-dynamic-param';
import { createPrerenderResumeDataCache, createRenderResumeDataCache } from '../resume-data-cache/resume-data-cache';
import { stringifyResumeDataCache } from '../resume-data-cache/resume-data-cache';
export var DynamicState = /*#__PURE__*/ function(DynamicState) {
    /**
   * The dynamic access occurred during the RSC render phase.
   */ DynamicState[DynamicState["DATA"] = 1] = "DATA";
    /**
   * The dynamic access occurred during the HTML shell render phase.
   */ DynamicState[DynamicState["HTML"] = 2] = "HTML";
    return DynamicState;
}({});
export var DynamicHTMLPreludeState = /*#__PURE__*/ function(DynamicHTMLPreludeState) {
    DynamicHTMLPreludeState[DynamicHTMLPreludeState["Empty"] = 0] = "Empty";
    DynamicHTMLPreludeState[DynamicHTMLPreludeState["Full"] = 1] = "Full";
    return DynamicHTMLPreludeState;
}({});
export async function getDynamicHTMLPostponedState(postponed, preludeState, fallbackRouteParams, resumeDataCache, isCacheComponentsEnabled) {
    const data = [
        preludeState,
        postponed
    ];
    const dataString = JSON.stringify(data);
    // If there are no fallback route params, we can just serialize the postponed
    // state as is.
    if (!fallbackRouteParams || fallbackRouteParams.size === 0) {
        // Serialized as `<postponedString.length>:<postponedString><renderResumeDataCache>`
        return `${dataString.length}:${dataString}${await stringifyResumeDataCache(createRenderResumeDataCache(resumeDataCache), isCacheComponentsEnabled)}`;
    }
    const replacements = Array.from(fallbackRouteParams.entries());
    const replacementsString = JSON.stringify(replacements);
    // Serialized as `<replacements.length><replacements><data>`
    const postponedString = `${replacementsString.length}${replacementsString}${dataString}`;
    // Serialized as `<postponedString.length>:<postponedString><renderResumeDataCache>`
    return `${postponedString.length}:${postponedString}${await stringifyResumeDataCache(resumeDataCache, isCacheComponentsEnabled)}`;
}
export async function getDynamicDataPostponedState(resumeDataCache, isCacheComponentsEnabled) {
    return `4:null${await stringifyResumeDataCache(createRenderResumeDataCache(resumeDataCache), isCacheComponentsEnabled)}`;
}
export function parsePostponedState(state, interpolatedParams, maxPostponedStateSizeBytes) {
    try {
        var _state_match;
        const postponedStringLengthMatch = (_state_match = state.match(/^([0-9]*):/)) == null ? void 0 : _state_match[1];
        if (!postponedStringLengthMatch) {
            throw Object.defineProperty(new Error(`Invariant: invalid postponed state ${state}`), "__NEXT_ERROR_CODE", {
                value: "E314",
                enumerable: false,
                configurable: true
            });
        }
        const postponedStringLength = parseInt(postponedStringLengthMatch);
        // We add a `:` to the end of the length as the first character of the
        // postponed string is the length of the replacement entries.
        const postponedString = state.slice(postponedStringLengthMatch.length + 1, postponedStringLengthMatch.length + postponedStringLength + 1);
        const renderResumeDataCache = createRenderResumeDataCache(state.slice(postponedStringLengthMatch.length + postponedStringLength + 1), maxPostponedStateSizeBytes);
        try {
            if (postponedString === 'null') {
                return {
                    type: 1,
                    renderResumeDataCache
                };
            }
            if (/^[0-9]/.test(postponedString)) {
                var _postponedString_match;
                const match = (_postponedString_match = postponedString.match(/^([0-9]*)/)) == null ? void 0 : _postponedString_match[1];
                if (!match) {
                    throw Object.defineProperty(new Error(`Invariant: invalid postponed state ${JSON.stringify(postponedString)}`), "__NEXT_ERROR_CODE", {
                        value: "E314",
                        enumerable: false,
                        configurable: true
                    });
                }
                // This is the length of the replacements entries.
                const length = parseInt(match);
                const replacements = JSON.parse(postponedString.slice(match.length, // We then go to the end of the string.
                match.length + length));
                let postponed = postponedString.slice(match.length + length);
                for (const [segmentKey, [searchValue, dynamicParamType]] of replacements){
                    const { treeSegment: [, // This is the same value that'll be used in the postponed state
                    // as it's part of the tree data. That's why we use it as the
                    // replacement value.
                    value] } = getDynamicParam(interpolatedParams, segmentKey, dynamicParamType, null);
                    postponed = postponed.replaceAll(searchValue, value);
                }
                return {
                    type: 2,
                    data: JSON.parse(postponed),
                    renderResumeDataCache
                };
            }
            return {
                type: 2,
                data: JSON.parse(postponedString),
                renderResumeDataCache
            };
        } catch (err) {
            console.error('Failed to parse postponed state', err);
            return {
                type: 1,
                renderResumeDataCache
            };
        }
    } catch (err) {
        console.error('Failed to parse postponed state', err);
        return {
            type: 1,
            renderResumeDataCache: createPrerenderResumeDataCache()
        };
    }
}
export function getPostponedFromState(state) {
    const [preludeState, postponed] = state.data;
    return {
        preludeState,
        postponed
    };
}

//# sourceMappingURL=postponed-state.js.map