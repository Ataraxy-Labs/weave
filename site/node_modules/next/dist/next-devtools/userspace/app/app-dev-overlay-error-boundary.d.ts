import { PureComponent } from 'react';
import type { GlobalErrorState } from '../../../client/components/app-router-instance';
type AppDevOverlayErrorBoundaryProps = {
    children: React.ReactNode;
    globalError: GlobalErrorState;
};
type AppDevOverlayErrorBoundaryState = {
    reactError: unknown;
};
export declare class AppDevOverlayErrorBoundary extends PureComponent<AppDevOverlayErrorBoundaryProps, AppDevOverlayErrorBoundaryState> {
    state: {
        reactError: null;
    };
    static getDerivedStateFromError(error: Error): {
        reactError: Error;
    };
    componentDidCatch(err: Error): void;
    render(): string | number | bigint | boolean | Iterable<import("react").ReactNode> | Promise<string | number | bigint | boolean | import("react").ReactElement<unknown, string | import("react").JSXElementConstructor<any>> | import("react").ReactPortal | Iterable<import("react").ReactNode> | null | undefined> | import("react/jsx-runtime").JSX.Element | null | undefined;
}
export {};
