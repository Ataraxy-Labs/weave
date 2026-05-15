import { jsx as _jsx } from "react/jsx-runtime";
import React from 'react';
import { loadGetInitialProps } from '../shared/lib/utils';
/**
 * `App` component is used for initialize of pages. It allows for overwriting and full control of the `page` initialization.
 * This allows for keeping state between navigation, custom error handling, injecting additional data.
 */ async function appGetInitialProps({ Component, ctx }) {
    const pageProps = await loadGetInitialProps(Component, ctx);
    return {
        pageProps
    };
}
export default class App extends React.Component {
    static{
        this.origGetInitialProps = appGetInitialProps;
    }
    static{
        this.getInitialProps = appGetInitialProps;
    }
    render() {
        const { Component, pageProps } = this.props;
        return /*#__PURE__*/ _jsx(Component, {
            ...pageProps
        });
    }
}

//# sourceMappingURL=_app.js.map