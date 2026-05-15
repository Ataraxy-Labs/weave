import './segment-boundary-trigger.css';
import type { SegmentBoundaryType, SegmentNodeState } from '../../../userspace/app/segment-explorer-node';
export declare function SegmentBoundaryTrigger({ nodeState, boundaries, }: {
    nodeState: SegmentNodeState;
    boundaries: Record<SegmentBoundaryType, string | null>;
}): import("react/jsx-runtime").JSX.Element;
