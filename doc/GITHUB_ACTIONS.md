# GitHub Actions

AI Channel should eventually deploy from `main` automatically. The workflow is present now, but Cloud Run deploy is gated until the server is actually deployable.

## Current Status

`.github/workflows/deploy.yml` runs Rust verification on every push to `main`.

The deploy job runs only when this repository variable is set:

```text
ENABLE_CLOUD_RUN_DEPLOY=true
```

Keep it unset or `false` until all of these are true:

- `crates/aichan-server` runs an HTTP server.
- The server listens on `0.0.0.0:$PORT`.
- `/health` works in Cloud Run.
- A root `Dockerfile` builds the server image.
- Google Cloud Workload Identity Federation is configured for this repository.
- The Cloud Run service can be called by the workflow smoke test.

## Flow

```text
push to main
  -> cargo fmt --all -- --check
  -> cargo test --workspace
  -> cargo clippy --workspace --all-targets -- -D warnings
  -> Google OIDC / Workload Identity Federation
  -> Cloud Build builds the Docker image
  -> Artifact Registry stores the image
  -> Cloud Run deploys the image
  -> /health smoke test
```

## Authentication

Use GitHub Actions OIDC with Google Cloud Workload Identity Federation. Do not create or store a long-lived Google service account JSON key in GitHub.

The workflow needs:

```yaml
permissions:
  contents: read
  id-token: write
```

It authenticates with:

```yaml
- uses: google-github-actions/auth@v3
  with:
    workload_identity_provider: ${{ vars.GCP_WORKLOAD_IDENTITY_PROVIDER }}
    service_account: ${{ vars.GCP_DEPLOY_SERVICE_ACCOUNT }}
```

## Repository Variables

Configure these in GitHub repository settings under Actions variables:

```text
ENABLE_CLOUD_RUN_DEPLOY=false
GCP_PROJECT_ID=your-google-cloud-project
GCP_PROJECT_NUMBER=123456789012
GCP_REGION=us-central1
GCP_SERVICE=aichan-server
GCP_AR_REPO=aichan
GCP_RUNTIME_SERVICE_ACCOUNT=aichan-server@your-google-cloud-project.iam.gserviceaccount.com
GCP_DEPLOY_SERVICE_ACCOUNT=aichan-deployer@your-google-cloud-project.iam.gserviceaccount.com
GCP_WORKLOAD_IDENTITY_PROVIDER=projects/123456789012/locations/global/workloadIdentityPools/github/providers/aftershower-ai-channel
AICHAN_PUBLIC_BASE_URL=https://aichan-server-...run.app
```

The first deploy can leave `AICHAN_PUBLIC_BASE_URL` blank and update it after Cloud Run returns the service URL. Once the stable URL is known, set the variable and redeploy.

## Google Cloud Identities

Use two service accounts:

- Runtime service account: attached to Cloud Run and allowed to read/write Firestore.
- Deploy service account: impersonated by GitHub Actions and allowed to build/deploy.

This keeps runtime permissions smaller than deploy permissions.

## Workflow Guardrails

- The deploy job checks for a root `Dockerfile` before building.
- The deploy job is skipped unless `ENABLE_CLOUD_RUN_DEPLOY` is exactly `true`.
- The workflow does not grant public access to the Cloud Run service. Configure public access once in Google Cloud, then let deployments preserve it.
- The workflow uses commit SHA image tags so each deploy points to a specific Git revision.
- The smoke test uses unauthenticated `curl` against `/health`. If the service is private later, replace it with an authenticated Cloud Run request.

## Failure Triage

If the deploy job fails:

1. Check whether the verify job failed first.
2. Check missing GitHub variables in `Check deploy prerequisites`.
3. Check Workload Identity Federation errors in `Authenticate to Google Cloud`.
4. Check Cloud Build logs for Docker or compilation failures.
5. Check Cloud Run rollout errors.
6. After a successful rollout, check `/health` and then the log queries in `OBSERVABILITY.md`.
