import { useEffect } from 'react';
import { isNextRouterError } from '../../../../client/components/is-next-router-error';
import { formatConsoleArgs, parseConsoleArgs } from '../../../../client/lib/console';
import isError from '../../../../lib/is-error';
import { createConsoleError } from '../../../shared/console-error';
import { coerceError, setOwnerStackIfAvailable } from './stitched-error';
import { forwardUnhandledError, logUnhandledRejection } from '../forward-logs';
const queueMicroTask = globalThis.queueMicrotask || ((cb)=>Promise.resolve().then(cb));
const errorQueue = [];
const errorHandlers = [];
const rejectionQueue = [];
const rejectionHandlers = [];
export function handleConsoleError(originError, consoleErrorArgs) {
    let error;
    const { environmentName } = parseConsoleArgs(consoleErrorArgs);
    if (isError(originError)) {
        error = createConsoleError(originError, environmentName);
    } else {
        error = createConsoleError(formatConsoleArgs(consoleErrorArgs), environmentName);
    }
    setOwnerStackIfAvailable(error);
    errorQueue.push(error);
    for (const handler of errorHandlers){
        // Delayed the error being passed to React Dev Overlay,
        // avoid the state being synchronously updated in the component.
        queueMicroTask(()=>{
            handler(error);
        });
    }
}
export function handleClientError(error) {
    errorQueue.push(error);
    for (const handler of errorHandlers){
        // Delayed the error being passed to React Dev Overlay,
        // avoid the state being synchronously updated in the component.
        queueMicroTask(()=>{
            handler(error);
        });
    }
}
export function useErrorHandler(handleOnUnhandledError, handleOnUnhandledRejection) {
    useEffect(()=>{
        // Handle queued errors.
        errorQueue.forEach(handleOnUnhandledError);
        rejectionQueue.forEach(handleOnUnhandledRejection);
        // Listen to new errors.
        errorHandlers.push(handleOnUnhandledError);
        rejectionHandlers.push(handleOnUnhandledRejection);
        return ()=>{
            // Remove listeners.
            errorHandlers.splice(errorHandlers.indexOf(handleOnUnhandledError), 1);
            rejectionHandlers.splice(rejectionHandlers.indexOf(handleOnUnhandledRejection), 1);
            // Reset error queues.
            errorQueue.splice(0, errorQueue.length);
            rejectionQueue.splice(0, rejectionQueue.length);
        };
    }, [
        handleOnUnhandledError,
        handleOnUnhandledRejection
    ]);
}
function onUnhandledError(event) {
    const thrownValue = event.error;
    if (isNextRouterError(thrownValue)) {
        event.preventDefault();
        return false;
    }
    // When there's an error property present, we log the error to error overlay.
    // Otherwise we don't do anything as it's not logging in the console either.
    if (thrownValue) {
        const error = coerceError(thrownValue);
        setOwnerStackIfAvailable(error);
        handleClientError(error);
        forwardUnhandledError(error);
    }
}
function onUnhandledRejection(ev) {
    const reason = ev?.reason;
    if (isNextRouterError(reason)) {
        ev.preventDefault();
        return;
    }
    const error = coerceError(reason);
    setOwnerStackIfAvailable(error);
    rejectionQueue.push(error);
    for (const handler of rejectionHandlers){
        handler(error);
    }
    logUnhandledRejection(reason);
}
export function handleGlobalErrors() {
    if (typeof window !== 'undefined') {
        try {
            // Increase the number of stack frames on the client
            Error.stackTraceLimit = 50;
        } catch  {}
        window.addEventListener('error', onUnhandledError);
        window.addEventListener('unhandledrejection', onUnhandledRejection);
    }
}

//# sourceMappingURL=use-error-handler.js.map