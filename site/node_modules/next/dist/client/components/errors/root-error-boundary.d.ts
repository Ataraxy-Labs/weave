import React, { type JSX } from 'react';
import { type ErrorBoundaryProps } from '../error-boundary';
export default function RootErrorBoundary({ children, errorComponent, errorStyles, errorScripts, }: ErrorBoundaryProps & {
    children: React.ReactNode;
}): JSX.Element;
