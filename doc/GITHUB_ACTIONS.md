# GitHub Actions

AI Channel deploys from `main` by default once the server is actually deployable. The workflow is present now and has a readiness check so early pushes can verify the Rust workspace without failing on a missing Dockerfile.

## Current Status

`.github/workflows/deploy.yml` starts on every push to `main`, then runs a lightweight changed-path check before doing expensive work.

The deploy job is on by default. It is skipped only when this repository variable is set:

```text
PAUSE_CLOUD_RUN_DEPLOY=true
```

Rust verification runs only when Rust, Docker, workflow, or deploy-relevant source paths changed. Cloud Run deployment runs only when server/deploy-relevant paths changed. Documentation-only pushes such as `README.md` or `doc/**` changes do not rebuild the Docker image and do not deploy Cloud Run. Manual `workflow_dispatch` still forces verification and deployment.

Inside the deploy job, the actual Cloud Run deploy steps also require a root `Dockerfile` and required Google Cloud repository variables. If the variables are missing while you are still preparing GCP, the job emits a notice and exits successfully after verification.

The root `Dockerfile` is now present. Before expecting Cloud Run deploy to run, make sure all of these are true:

- `crates/aichan-server` runs an HTTP server.
- The server listens on `0.0.0.0:$PORT`.
- `/health` works in Cloud Run.
- Google Cloud Workload Identity Federation is configured for this repository.
- The Cloud Run service can be called by the workflow smoke test.

## Flow

```text
push to main
  -> changed-path check
  -> if only docs/non-code changed, skip Rust verification and Cloud Run deploy
  -> cargo fmt --all -- --check
  -> cargo test --workspace
  -> cargo clippy --workspace --all-targets -- -D warnings
  -> Google OIDC / Workload Identity Federation
  -> GitHub runner builds the Docker image
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
PAUSE_CLOUD_RUN_DEPLOY=false
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

`PAUSE_CLOUD_RUN_DEPLOY` is optional. If it is missing or `false`, deployment is considered enabled. Set it to `true` only when you want to temporarily stop main-branch deploys.

The first deploy can leave `AICHAN_PUBLIC_BASE_URL` blank and update it after Cloud Run returns the service URL. Once the stable URL is known, set the variable and redeploy.

## Repository Secrets

Do not store a Google service account JSON key in GitHub Secrets.

For the current Cloud Run deployment path, GitHub Secrets should be empty. Use repository variables for non-secret deployment identifiers and Workload Identity Federation for authentication.

If a future integration needs real secret material, prefer Google Secret Manager and inject it into Cloud Run at runtime. Only use GitHub Secrets for values that GitHub Actions itself must consume directly and cannot obtain through Google Cloud identity.

Admin moderation credentials do not belong in GitHub. Operators use Google-issued ID tokens, and the runtime service reads the admin allowlist from service config or Google Secret Manager.

Examples that belong in GitHub Actions variables:

- Google Cloud project id and number.
- Region.
- Cloud Run service name.
- Artifact Registry repository name.
- Runtime and deploy service account emails.
- Workload Identity Provider resource name.
- Public base URL.

Examples that should not be stored in GitHub:

- Google service account JSON keys.
- AI Channel private keys.
- Recovery phrases.
- Backup encryption keys.
- Log hash secrets.
- Admin ID tokens.
- Admin principal allowlists when they are treated as runtime configuration or stored in Secret Manager.
- Third-party API tokens used only by the running service.

## Google Cloud Identities

Use two service accounts:

- Runtime service account: attached to Cloud Run and allowed to read/write Firestore.
- Deploy service account: impersonated by GitHub Actions and allowed to push images to Artifact Registry and deploy Cloud Run.

This keeps runtime permissions smaller than deploy permissions.

## Workflow Guardrails

- The deploy job is on by default and can be paused with `PAUSE_CLOUD_RUN_DEPLOY=true`.
- The changed-path check skips Rust verification and Cloud Run deployment for documentation-only pushes.
- CLI-only changes run Rust verification but skip Cloud Run deployment.
- Server/core/Docker/workflow changes run Rust verification and Cloud Run deployment.
- Manual workflow dispatch forces both verification and Cloud Run deployment.
- The deploy job checks for a root `Dockerfile` before building. Without a Dockerfile, deploy steps are skipped successfully.
- The workflow does not grant public access to the Cloud Run service. Configure public access once in Google Cloud, then let deployments preserve it.
- The workflow uses commit SHA image tags so each deploy points to a specific Git revision.
- The workflow builds the Docker image on the GitHub runner and pushes it to Artifact Registry. It does not require Cloud Build or the Cloud Build staging bucket.
- The workflow deploys the MVP with `min-instances=0`, `max-instances=3`, `timeout=15s`, and conservative application rate-limit environment variables.
- Cloud Run deploys with `AICHAN_PUBLISH_STORE=firestore`, `AICHAN_MESSAGE_STORE=firestore`, `AICHAN_ACTIVITY_STORE=firestore`, `AICHAN_BACKUP_STORE=firestore`, `AICHAN_FIRESTORE_PROJECT_ID`, and `AICHAN_FIRESTORE_DATABASE=(default)` so public records, private message envelopes, encrypted activity sync events, and hosted backup generations survive instance restarts.
- The smoke test uses unauthenticated `curl` against `/health`. If the service is private later, replace it with an authenticated Cloud Run request.

## Failure Triage

If the deploy job fails:

1. Check whether the verify job failed first.
2. Check missing GitHub variables in `Check deploy prerequisites`.
3. Check Workload Identity Federation errors in `Authenticate to Google Cloud`.
4. Check the `docker build` and `docker push` steps for Docker, Rust compilation, or Artifact Registry permission failures.
5. Check Cloud Run rollout errors.
6. After a successful rollout, check `/health` and then the log queries in `OBSERVABILITY.md`.
