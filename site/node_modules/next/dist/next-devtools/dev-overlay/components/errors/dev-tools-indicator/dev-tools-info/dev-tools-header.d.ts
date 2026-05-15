import React from 'react';
interface DevToolsHeaderProps {
    title: React.ReactNode;
    children?: React.ReactNode;
}
export declare function DevToolsHeader({ title, children, ref, }: DevToolsHeaderProps & {
    ref?: React.Ref<HTMLDivElement>;
}): import("react/jsx-runtime").JSX.Element;
export {};
