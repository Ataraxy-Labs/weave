import s from 'next/dist/compiled/superstruct';
const dynamicParamTypesSchema = s.enums([
    'c',
    'ci(..)(..)',
    'ci(.)',
    'ci(..)',
    'ci(...)',
    'oc',
    'd',
    'di(..)(..)',
    'di(.)',
    'di(..)',
    'di(...)'
]);
const segmentSchema = s.union([
    s.string(),
    s.tuple([
        // Param name
        s.string(),
        // Param cache key (almost the same as the value, but arrays are
        // concatenated into strings)
        // TODO: We should change this to just be the value. Currently we convert
        // it back to a value when passing to useParams. It only needs to be
        // a string when converted to a a cache key, but that doesn't mean we
        // need to store it as that representation.
        s.string(),
        // Dynamic param type
        dynamicParamTypesSchema
    ])
]);
// unfortunately the tuple is not understood well by Describe so we have to
// use any here. This does not have any impact on the runtime type since the validation
// does work correctly.
export const flightRouterStateSchema = s.tuple([
    segmentSchema,
    s.record(s.string(), s.lazy(()=>flightRouterStateSchema)),
    s.optional(s.nullable(s.string())),
    s.optional(s.nullable(s.union([
        s.literal('refetch'),
        s.literal('refresh'),
        s.literal('inside-shared-layout'),
        s.literal('metadata-only')
    ]))),
    s.optional(s.boolean())
]);

//# sourceMappingURL=types.js.map