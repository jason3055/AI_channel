# Google Cloud Deployment References

These notes summarize official deployment references used by `doc/DEPLOYMENT.md`.

## Sources

- [gcloud services enable](https://cloud.google.com/sdk/gcloud/reference/services/enable): enables one or more APIs for a project.
- [Get started with Cloud Firestore](https://firebase.google.com/docs/firestore/quickstart): create a Firebase project or add Firebase to a Google Cloud project, then create a Firestore database and choose its location.
- [gcloud firestore databases create](https://cloud.google.com/sdk/gcloud/reference/firestore/databases/create): creates a Firestore Native database with `--location`, optional `--database`, and `--type=firestore-native`.
- [Manage data retention with TTL policies](https://firebase.google.com/docs/firestore/ttl): Firestore TTL uses a timestamp field, is not instantaneous, and commonly deletes expired data within 24 hours.
- [gcloud firestore fields ttls update](https://cloud.google.com/sdk/gcloud/reference/firestore/fields/ttls/update): enables or disables a TTL field for a collection group.
- [gcloud firestore indexes composite create](https://cloud.google.com/sdk/gcloud/reference/firestore/indexes/composite/create): creates composite indexes, including array `contains` fields and ordered fields.
- [Cloud Run container runtime contract](https://cloud.google.com/run/docs/container-contract): services must listen on `0.0.0.0` and use the injected `PORT` environment variable.
- [Cloud Run service identity](https://cloud.google.com/run/docs/securing/service-identity): prefer a user-managed service account, avoid `GOOGLE_APPLICATION_CREDENTIALS` on Cloud Run, and obtain OAuth access tokens from the metadata server for Google APIs.
- [Configure service identity for services](https://cloud.google.com/run/docs/configuring/services/service-identity): attach a service account to a Cloud Run service.
- [Workload Identity Federation from GitHub Actions](https://github.com/google-github-actions/auth): GitHub Actions can authenticate to Google Cloud through OIDC without long-lived service account keys.
- [Deploy to Cloud Run from GitHub Actions](https://github.com/google-github-actions/deploy-cloudrun): deploys a built image or source directory to Cloud Run from a workflow.
- [Set up gcloud in GitHub Actions](https://github.com/google-github-actions/setup-gcloud): installs and configures the Google Cloud CLI in a workflow.
- [Firestore server client library security](https://cloud.google.com/firestore/docs/security/iam): server access is secured by IAM and Firestore roles such as `roles/datastore.user`.
- [Firestore SDKs and client libraries](https://firebase.google.com/docs/firestore/client/libraries): server client libraries use privileged environments and are not evaluated against Security Rules.
- [Firestore REST `documents:runQuery`](https://firebase.google.com/docs/firestore/reference/rest/v1/projects.databases.documents/runQuery): runs structured queries against Firestore documents.
- [Firestore REST `documents:commit`](https://cloud.google.com/firestore/docs/reference/rest/v1/projects.databases.documents/commit): atomically applies document writes and preconditions.
- [Build and push a Docker image with Cloud Build](https://cloud.google.com/build/docs/build-push-docker-image): build a Dockerfile and push the resulting image to Artifact Registry.
- [Deploying container images to Cloud Run](https://cloud.google.com/run/docs/deploying): deploy an image with `gcloud run deploy SERVICE --image IMAGE_URL`.
- [Allowing public access to Cloud Run](https://cloud.google.com/run/docs/authenticating/public): public services can disable Invoker IAM check or grant unauthenticated invoker access.
- [Firebase Hosting rewrites to Cloud Run](https://firebase.google.com/docs/hosting/cloud-run): Hosting can route HTTPS requests to Cloud Run services with rewrites.
- [Cloud Run custom domains](https://cloud.google.com/run/docs/mapping-custom-domains): recommended custom domain options include a global external Application Load Balancer or Firebase Hosting; Cloud Run domain mappings are preview and limited.

## Local Decisions

- Use Firestore Native mode, not Datastore mode.
- Use Cloud Run as the public HTTP surface.
- Use Firebase Hosting only as an optional front door later.
- Use server-side IAM instead of Firebase client SDK access.
- Use GitHub Actions OIDC and Workload Identity Federation instead of service account JSON keys.
- Use Firestore TTL as cleanup help, not as the only expiration check.
- Keep the first deployment inexpensive: one region, `min_instances = 0`, bounded `max_instances`.
