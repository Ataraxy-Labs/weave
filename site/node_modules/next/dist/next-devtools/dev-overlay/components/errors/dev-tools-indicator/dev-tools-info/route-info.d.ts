import type { ComponentProps } from 'react';
export declare const learnMoreLink: {
    readonly pages: {
        readonly static: "https://nextjs.org/docs/pages/building-your-application/rendering/static-site-generation";
        readonly dynamic: "https://nextjs.org/docs/pages/building-your-application/rendering/server-side-rendering";
    };
    readonly app: {
        readonly static: "https://nextjs.org/docs/app/building-your-application/rendering/server-components#static-rendering-default";
        readonly dynamic: "https://nextjs.org/docs/app/building-your-application/rendering/server-components#dynamic-rendering";
    };
};
export declare function RouteInfoBody({ routerType, isStaticRoute, ...props }: {
    routerType: 'pages' | 'app';
    isStaticRoute: boolean;
} & ComponentProps<'div'>): import("react/jsx-runtime").JSX.Element;
