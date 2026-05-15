import { webpack } from 'next/dist/compiled/webpack/webpack';
export default class ForceCompleteRuntimePlugin {
    allSharedRuntimeGlobals: Set<"__webpack_require__.n">;
    apply(compiler: webpack.Compiler): void;
}
