import * as React from 'react';
export type OverlayProps = React.HTMLAttributes<HTMLDivElement> & {
    fixed?: boolean;
    ref?: React.Ref<HTMLDivElement>;
};
declare const Overlay: React.FC<OverlayProps>;
export { Overlay };
