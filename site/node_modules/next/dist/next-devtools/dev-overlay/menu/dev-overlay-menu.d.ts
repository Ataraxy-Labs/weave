export declare const DevtoolMenu: ({ closeOnClickOutside, items, }: {
    closeOnClickOutside?: boolean;
    items: Array<false | undefined | null | {
        onClick?: () => void;
        title?: string;
        label: string;
        value: React.ReactNode;
        attributes?: Record<string, string | boolean>;
        footer?: boolean;
    }>;
}) => import("react/jsx-runtime").JSX.Element;
export declare function IssueCount({ children }: {
    children: number;
}): import("react/jsx-runtime").JSX.Element;
export declare function ChevronRight(): import("react/jsx-runtime").JSX.Element;
