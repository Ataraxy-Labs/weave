// This could also be a variable instead of a function, but some unit tests want to change the ID at
// runtime. Even though that would never happen in a real deployment.
export function getDeploymentId() {
    return process.env.NEXT_DEPLOYMENT_ID;
}
export function getDeploymentIdQueryOrEmptyString() {
    let deploymentId = getDeploymentId();
    if (deploymentId) {
        return `?dpl=${deploymentId}`;
    }
    return '';
}

//# sourceMappingURL=deployment-id.js.map