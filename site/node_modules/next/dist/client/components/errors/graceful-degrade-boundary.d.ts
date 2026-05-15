import { Component, type ReactNode } from 'react';
interface ErrorBoundaryProps {
    children: ReactNode;
}
interface ErrorBoundaryState {
    hasError: boolean;
}
export declare class GracefulDegradeBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
    private rootHtml;
    private htmlAttributes;
    private htmlRef;
    constructor(props: ErrorBoundaryProps);
    static getDerivedStateFromError(_: unknown): ErrorBoundaryState;
    componentDidMount(): void;
    render(): string | number | bigint | boolean | Iterable<ReactNode> | Promise<string | number | bigint | boolean | import("react").ReactElement<unknown, string | import("react").JSXElementConstructor<any>> | import("react").ReactPortal | Iterable<ReactNode> | null | undefined> | import("react/jsx-runtime").JSX.Element | null | undefined;
}
export default GracefulDegradeBoundary;
