import './dynamic-panel.css';
export declare function DynamicPanel({ header, children, draggable, sizeConfig, closeOnClickOutside, sharePanelSizeGlobally, sharePanelPositionGlobally, containerProps, }: {
    header: React.ReactNode;
    children: React.ReactNode;
    draggable?: boolean;
    sharePanelSizeGlobally?: boolean;
    sharePanelPositionGlobally?: boolean;
    containerProps?: React.HTMLProps<HTMLDivElement>;
    sizeConfig?: {
        kind: 'resizable';
        minWidth: string | number;
        minHeight: string | number;
        maxWidth: string | number;
        maxHeight: string | number;
        initialSize: {
            height: number;
            width: number;
        };
        sides?: Array<'horizontal' | 'vertical' | 'diagonal'>;
    } | {
        kind: 'fixed';
        height: number;
        width: number;
    };
    closeOnClickOutside?: boolean;
}): import("react/jsx-runtime").JSX.Element;
