import React from 'react';
import isError from '../../../../lib/is-error';
const ownerStacks = new WeakMap();
export function getOwnerStack(error) {
    return ownerStacks.get(error);
}
export function setOwnerStack(error, stack) {
    ownerStacks.set(error, stack);
}
export function coerceError(value) {
    return isError(value) ? value : Object.defineProperty(new Error('' + value), "__NEXT_ERROR_CODE", {
        value: "E394",
        enumerable: false,
        configurable: true
    });
}
export function setOwnerStackIfAvailable(error) {
    // React 18 and prod does not have `captureOwnerStack`
    if ('captureOwnerStack' in React) {
        setOwnerStack(error, React.captureOwnerStack());
    }
}
export function decorateDevError(thrownValue) {
    const error = coerceError(thrownValue);
    setOwnerStackIfAvailable(error);
    return error;
}

//# sourceMappingURL=stitched-error.js.map