import { type AppRouterActionQueue, type GlobalErrorState } from './app-router-instance';
import type { StaticIndicatorState } from '../dev/hot-reloader/app/hot-reloader-app';
export default function AppRouter({ actionQueue, globalErrorState, webSocket, staticIndicatorState, }: {
    actionQueue: AppRouterActionQueue;
    globalErrorState: GlobalErrorState;
    webSocket?: WebSocket;
    staticIndicatorState?: StaticIndicatorState;
}): import("react/jsx-runtime").JSX.Element;
