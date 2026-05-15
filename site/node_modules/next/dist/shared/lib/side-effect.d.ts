import type React from 'react';
import { type JSX } from 'react';
type State = JSX.Element[] | undefined;
export type SideEffectProps = {
    reduceComponentsToState: (components: Array<React.ReactElement<any>>) => State;
    handleStateChange?: (state: State) => void;
    headManager: any;
    children: React.ReactNode;
};
export default function SideEffect(props: SideEffectProps): null;
export {};
