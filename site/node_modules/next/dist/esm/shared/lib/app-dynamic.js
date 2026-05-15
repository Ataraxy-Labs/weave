import Loadable from './lazy-dynamic/loadable';
export default function dynamic(dynamicOptions, options) {
    const loadableOptions = {};
    if (typeof dynamicOptions === 'function') {
        loadableOptions.loader = dynamicOptions;
    }
    const mergedOptions = {
        ...loadableOptions,
        ...options
    };
    return Loadable({
        ...mergedOptions,
        modules: mergedOptions.loadableGenerated?.modules
    });
}

//# sourceMappingURL=app-dynamic.js.map