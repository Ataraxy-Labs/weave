'use client';
import { jsx as _jsx } from "react/jsx-runtime";
import React, { useEffect } from 'react';
import { useRouter } from './navigation';
import { getRedirectTypeFromError, getURLFromRedirectError } from './redirect';
import { RedirectType, isRedirectError } from './redirect-error';
function HandleRedirect({ redirect, reset, redirectType }) {
    const router = useRouter();
    useEffect(()=>{
        React.startTransition(()=>{
            if (redirectType === RedirectType.push) {
                router.push(redirect, {});
            } else {
                router.replace(redirect, {});
            }
            reset();
        });
    }, [
        redirect,
        redirectType,
        reset,
        router
    ]);
    return null;
}
export class RedirectErrorBoundary extends React.Component {
    constructor(props){
        super(props);
        this.state = {
            redirect: null,
            redirectType: null
        };
    }
    static getDerivedStateFromError(error) {
        if (isRedirectError(error)) {
            const url = getURLFromRedirectError(error);
            const redirectType = getRedirectTypeFromError(error);
            if ('handled' in error) {
                // The redirect was already handled. We'll still catch the redirect error
                // so that we can remount the subtree, but we don't actually need to trigger the
                // router.push.
                return {
                    redirect: null,
                    redirectType: null
                };
            }
            return {
                redirect: url,
                redirectType
            };
        }
        // Re-throw if error is not for redirect
        throw error;
    }
    // Explicit type is needed to avoid the generated `.d.ts` having a wide return type that could be specific to the `@types/react` version.
    render() {
        const { redirect, redirectType } = this.state;
        if (redirect !== null && redirectType !== null) {
            return /*#__PURE__*/ _jsx(HandleRedirect, {
                redirect: redirect,
                redirectType: redirectType,
                reset: ()=>this.setState({
                        redirect: null
                    })
            });
        }
        return this.props.children;
    }
}
export function RedirectBoundary({ children }) {
    const router = useRouter();
    return /*#__PURE__*/ _jsx(RedirectErrorBoundary, {
        router: router,
        children: children
    });
}

//# sourceMappingURL=redirect-boundary.js.map