import type { Corners } from '../../../shared';
export declare function Draggable({ children, padding, position: currentCorner, setPosition: setCurrentCorner, onDragStart, dragHandleSelector, disableDrag, avoidZone, ...props }: {
    children: React.ReactElement;
    position: Corners;
    padding: number;
    setPosition: (position: Corners) => void;
    onDragStart?: () => void;
    dragHandleSelector?: string;
    disableDrag?: boolean;
    style?: React.CSSProperties;
    avoidZone?: {
        square: number;
        corner: Corners;
        padding: number;
    };
}): import("react/jsx-runtime").JSX.Element;
