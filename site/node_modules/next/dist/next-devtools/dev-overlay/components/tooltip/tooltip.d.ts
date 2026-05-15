import './tooltip.css';
type TooltipDirection = 'top' | 'bottom' | 'left' | 'right';
interface TooltipProps {
    children: React.ReactNode;
    title: string | null;
    direction?: TooltipDirection;
    arrowSize?: number;
    offset?: number;
    className?: string;
}
export declare const Tooltip: import("react").ForwardRefExoticComponent<TooltipProps & import("react").RefAttributes<HTMLDivElement>>;
export {};
