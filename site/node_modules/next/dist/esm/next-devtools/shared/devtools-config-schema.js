import { z } from 'next/dist/compiled/zod';
export const devToolsConfigSchema = z.object({
    theme: z.enum([
        'light',
        'dark',
        'system'
    ]).optional(),
    disableDevIndicator: z.boolean().optional(),
    devToolsPosition: z.enum([
        'top-left',
        'top-right',
        'bottom-left',
        'bottom-right'
    ]).optional(),
    devToolsPanelPosition: z.record(z.string(), z.enum([
        'top-left',
        'top-right',
        'bottom-left',
        'bottom-right'
    ])).optional(),
    devToolsPanelSize: z.record(z.string(), z.object({
        width: z.number(),
        height: z.number()
    })).optional(),
    scale: z.number().optional(),
    hideShortcut: z.string().nullable().optional()
});

//# sourceMappingURL=devtools-config-schema.js.map