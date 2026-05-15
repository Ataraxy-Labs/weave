import React from 'react';
type PagesDevOverlayErrorBoundaryProps = {
    children?: React.ReactNode;
};
type PagesDevOverlayErrorBoundaryState = {
    error: Error | null;
};
export declare class PagesDevOverlayErrorBoundary extends React.PureComponent<PagesDevOverlayErrorBoundaryProps, PagesDevOverlayErrorBoundaryState> {
    state: {
        error: null;
    };
    static getDerivedStateFromError(error: Error): {
        error: Error;
    };
    render(): React.ReactNode;
}
export {};
