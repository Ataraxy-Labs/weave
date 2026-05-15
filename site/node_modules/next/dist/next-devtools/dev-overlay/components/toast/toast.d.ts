import * as React from 'react';
type ToastProps = React.HTMLProps<HTMLDivElement> & {
    children?: React.ReactNode;
    onClick?: () => void;
    className?: string;
};
export declare const Toast: React.ForwardRefExoticComponent<Omit<ToastProps, "ref"> & React.RefAttributes<HTMLDivElement>>;
export {};
