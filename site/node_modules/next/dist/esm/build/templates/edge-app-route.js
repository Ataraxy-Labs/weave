var _self___RSC_MANIFEST;
import { setManifestsSingleton } from '../../server/app-render/manifests-singleton';
import { EdgeRouteModuleWrapper } from '../../server/web/edge-route-module-wrapper';
// Import the userland code.
import * as module from 'VAR_USERLAND';
// injected by the loader afterwards.
const maybeJSONParse = (str)=>str ? JSON.parse(str) : undefined;
const rscManifest = (_self___RSC_MANIFEST = self.__RSC_MANIFEST) == null ? void 0 : _self___RSC_MANIFEST['VAR_PAGE'];
const rscServerManifest = maybeJSONParse(self.__RSC_SERVER_MANIFEST);
if (rscManifest && rscServerManifest) {
    setManifestsSingleton({
        page: 'VAR_PAGE',
        clientReferenceManifest: rscManifest,
        serverActionsManifest: rscServerManifest
    });
}
export const ComponentMod = module;
const handler = EdgeRouteModuleWrapper.wrap(module.routeModule, {
    page: 'VAR_PAGE'
});
export default handler;

//# sourceMappingURL=edge-app-route.js.map