import * as React from 'react';
type DialogProps = {
    children?: React.ReactNode;
    'aria-labelledby': string;
    'aria-describedby': string;
    className?: string;
    onClose?: () => void;
} & React.HTMLAttributes<HTMLDivElement>;
declare const Dialog: React.FC<DialogProps>;
export { Dialog };
