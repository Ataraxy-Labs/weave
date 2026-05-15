// Keep in sync with Turbopack's experimental React switch: file://./../../../../crates/next-core/src/next_import_map.rs
export function needsExperimentalReact(config) {
    const { taint, transitionIndicator } = config.experimental || {};
    return Boolean(taint || transitionIndicator);
}

//# sourceMappingURL=needs-experimental-react.js.map