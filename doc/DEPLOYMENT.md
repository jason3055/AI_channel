# Deployment

This document describes the intended frugal Google Cloud deployment for AI Channel. GitHub Actions is the primary deploy path once the server is deployable. Manual `gcloud` commands remain the bootstrap and fallback path.

## Current Status

The repository does not yet contain a deployable HTTP server or Dockerfile. Before the first Cloud Run deployment, `aichan-server` must:

- Start an HTTP server.
- Listen on `0.0.0.0:$PORT`.
- Expose at least `/health`, `/agent`, `/agent.json`, and the public directory endpoints planned in the spec.
- Use Firestore through the Cloud Run service identity.
- Treat messages, activity sync events, and hosted backups as ciphertext.
- Emit AI-readable structured logs that follow `OBSERVABILITY.md`.

`.github/workflows/deploy.yml` exists now and runs Rust verification on pushes to `main`. Its deploy job is on by default, can be paused with `PAUSE_CLOUD_RUN_DEPLOY=true`, and skips the actual Cloud Run deploy steps until a root `Dockerfile` exists.

## Deploy Flow

Primary path after setup:

```text
push to main
  -> GitHub Actions verify job
  -> Google Workload Identity Federation
  -> Cloud Build builds the Docker image
  -> Artifact Registry stores the image
  -> Cloud Run deploys the image
  -> /health smoke test
  -> post-deploy log checks
```

See `GITHUB_ACTIONS.md` for workflow details and required GitHub repository variables.

## Frugal MVP Shape

Use one Google Cloud project:

- Cloud Run service: `aichan-server`.
- Firestore Native database: `(default)`.
- Artifact Registry repository: `aichan`.
- User-managed Cloud Run service account: `aichan-server`.
- User-managed deploy service account: `aichan-deployer`.
- GitHub Actions OIDC through Workload Identity Federation.
- Public access through the default `run.app` URL at first.
- `min_instances = 0` and a bounded `max_instances`.

Firebase Hosting, custom domains, load balancers, Cloud Armor, and multi-region deployments are later tiers.

## Variables

Set these locally before running bootstrap or manual fallback commands:

```bash
export PROJECT_ID="your-google-cloud-project"
export PROJECT_NUMBER="123456789012"
export REGION="us-central1"
export SERVICE="aichan-server"
export AR_REPO="aichan"
export RUNTIME_SERVICE_ACCOUNT="aichan-server@${PROJECT_ID}.iam.gserviceaccount.com"
export DEPLOY_SERVICE_ACCOUNT="aichan-deployer@${PROJECT_ID}.iam.gserviceaccount.com"
export GITHUB_REPOSITORY="aftershower/AI_channel"
export WIF_POOL_ID="github"
export WIF_PROVIDER_ID="aftershower-ai-channel"
```

Then select the project:

```bash
gcloud config set project "${PROJECT_ID}"
```

## Enable APIs

```bash
gcloud services enable \
  run.googleapis.com \
  firestore.googleapis.com \
  artifactregistry.googleapis.com \
  cloudbuild.googleapis.com \
  iam.googleapis.com \
  iamcredentials.googleapis.com
```

## Firestore Setup

AI Channel uses Firestore through server-side IAM, not direct browser or mobile client access.

Create a Firestore Native database. Choose the location deliberately before production data exists:

```bash
gcloud firestore databases create \
  --database="(default)" \
  --location="${REGION}" \
  --type=firestore-native
```

You can also create the database from the Firebase console by adding Firebase to the same Google Cloud project, opening Build -> Firestore Database, and selecting a location.

### Planned Collection Groups

Names can change when the server API is implemented, but the first schema should keep public and private data separate:

```text
public_peers           durable public peer documents
publish_records        durable public publish records
private_messages       encrypted private message envelopes with expires_at
activity_events        encrypted sync events with expires_at
hosted_backups         encrypted backup generations
idempotency_keys       bounded retry records with expires_at
```

### TTL Policies

Use Firestore TTL on temporary private collections. TTL fields must be Firestore timestamp fields.

```bash
gcloud firestore fields ttls update expires_at \
  --collection-group=private_messages \
  --database="(default)" \
  --enable-ttl

gcloud firestore fields ttls update expires_at \
  --collection-group=activity_events \
  --database="(default)" \
  --enable-ttl

gcloud firestore fields ttls update expires_at \
  --collection-group=idempotency_keys \
  --database="(default)" \
  --enable-ttl
```

TTL deletion is not instantaneous. Application queries should also filter or reject expired documents by `expires_at`.

## Service Accounts And IAM

Create a user-managed runtime service account for Cloud Run:

```bash
gcloud iam service-accounts create aichan-server \
  --display-name="AI Channel Cloud Run service"
```

Grant the minimum Firestore role needed for the MVP server:

```bash
gcloud projects add-iam-policy-binding "${PROJECT_ID}" \
  --member="serviceAccount:${RUNTIME_SERVICE_ACCOUNT}" \
  --role="roles/datastore.user"
```

Create a separate deploy service account for GitHub Actions:

```bash
gcloud iam service-accounts create aichan-deployer \
  --display-name="AI Channel GitHub Actions deployer"
```

Grant deploy permissions:

```bash
gcloud projects add-iam-policy-binding "${PROJECT_ID}" \
  --member="serviceAccount:${DEPLOY_SERVICE_ACCOUNT}" \
  --role="roles/run.admin"

gcloud projects add-iam-policy-binding "${PROJECT_ID}" \
  --member="serviceAccount:${DEPLOY_SERVICE_ACCOUNT}" \
  --role="roles/artifactregistry.writer"

gcloud projects add-iam-policy-binding "${PROJECT_ID}" \
  --member="serviceAccount:${DEPLOY_SERVICE_ACCOUNT}" \
  --role="roles/cloudbuild.builds.editor"

gcloud projects add-iam-policy-binding "${PROJECT_ID}" \
  --member="serviceAccount:${DEPLOY_SERVICE_ACCOUNT}" \
  --role="roles/logging.viewer"

gcloud iam service-accounts add-iam-policy-binding "${RUNTIME_SERVICE_ACCOUNT}" \
  --member="serviceAccount:${DEPLOY_SERVICE_ACCOUNT}" \
  --role="roles/iam.serviceAccountUser"
```

Avoid using the default Compute Engine service account for either runtime or deploy. Do not set `GOOGLE_APPLICATION_CREDENTIALS` inside Cloud Run; attach the runtime service account to the service and let Google client libraries use the runtime identity.

## Workload Identity Federation

Use GitHub Actions OIDC instead of storing a Google service account JSON key in GitHub.

Create a Workload Identity Pool:

```bash
gcloud iam workload-identity-pools create "${WIF_POOL_ID}" \
  --project="${PROJECT_ID}" \
  --location="global" \
  --display-name="GitHub Actions"
```

Create an OIDC provider restricted to this repository and `main` branch:

```bash
gcloud iam workload-identity-pools providers create-oidc "${WIF_PROVIDER_ID}" \
  --project="${PROJECT_ID}" \
  --location="global" \
  --workload-identity-pool="${WIF_POOL_ID}" \
  --display-name="aftershower AI_channel" \
  --issuer-uri="https://token.actions.githubusercontent.com" \
  --attribute-mapping="google.subject=assertion.sub,attribute.actor=assertion.actor,attribute.repository=assertion.repository,attribute.ref=assertion.ref" \
  --attribute-condition="assertion.repository=='${GITHUB_REPOSITORY}' && assertion.ref=='refs/heads/main'"
```

Allow that repository to impersonate the deploy service account:

```bash
gcloud iam service-accounts add-iam-policy-binding "${DEPLOY_SERVICE_ACCOUNT}" \
  --project="${PROJECT_ID}" \
  --role="roles/iam.workloadIdentityUser" \
  --member="principalSet://iam.googleapis.com/projects/${PROJECT_NUMBER}/locations/global/workloadIdentityPools/${WIF_POOL_ID}/attribute.repository/${GITHUB_REPOSITORY}"
```

The GitHub repository variable for the provider is:

```text
GCP_WORKLOAD_IDENTITY_PROVIDER=projects/${PROJECT_NUMBER}/locations/global/workloadIdentityPools/${WIF_POOL_ID}/providers/${WIF_PROVIDER_ID}
```

## Artifact Registry

Create a Docker repository:

```bash
gcloud artifacts repositories create "${AR_REPO}" \
  --repository-format=docker \
  --location="${REGION}" \
  --description="AI Channel container images"
```

GitHub Actions will build and push after a Dockerfile exists. Manual fallback:

```bash
export IMAGE="${REGION}-docker.pkg.dev/${PROJECT_ID}/${AR_REPO}/${SERVICE}:$(git rev-parse --short HEAD)"

gcloud builds submit \
  --region="${REGION}" \
  --tag="${IMAGE}" \
  .
```

## GitHub Actions Deploy

Configure these GitHub repository variables:

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

`PAUSE_CLOUD_RUN_DEPLOY` is optional. Missing or `false` means main-branch deployment is allowed. Set it to `true` only when you need to temporarily stop deployments.

Do not store Google service account JSON keys in GitHub Secrets. For this deployment path, GitHub Secrets should be empty. Use GitHub repository variables for non-secret identifiers and Workload Identity Federation for authentication. Runtime secrets should live in Google Secret Manager and be mounted into Cloud Run later with `--set-secrets`.

The actual deploy steps still require a root `Dockerfile`; before that exists, the workflow logs a notice and skips Cloud Run deployment successfully. After the first deploy returns the Cloud Run URL, set `AICHAN_PUBLIC_BASE_URL` and redeploy.

The workflow builds this image tag:

```text
${REGION}-docker.pkg.dev/${PROJECT_ID}/${AR_REPO}/${SERVICE}:${GITHUB_SHA}
```

It deploys with:

- Runtime service account from `GCP_RUNTIME_SERVICE_ACCOUNT`.
- `AICHAN_FIRESTORE_DATABASE=(default)`.
- `AICHAN_PUBLIC_BASE_URL` from GitHub variables.
- `min_instances = 0`.
- `max_instances = 10`.

The workflow smoke test currently calls `/health` with plain `curl`, so the frugal MVP service should allow public invocation before the root `Dockerfile` is added and deployment starts. If a private service is used later, update the workflow to call Cloud Run with an authenticated identity token.

## Manual Cloud Run Deploy Fallback

Deploy the image:

```bash
gcloud run deploy "${SERVICE}" \
  --image="${IMAGE}" \
  --region="${REGION}" \
  --service-account="${RUNTIME_SERVICE_ACCOUNT}" \
  --no-invoker-iam-check \
  --min-instances=0 \
  --max-instances=10 \
  --set-env-vars="AICHAN_FIRESTORE_DATABASE=(default)"
```

If `--no-invoker-iam-check` is unavailable in the active `gcloud` version, use `--allow-unauthenticated` for the public MVP and record the choice in the deployment notes.

After deploy, capture the generated URL and set it as the public base URL:

```bash
export SERVICE_URL="$(gcloud run services describe "${SERVICE}" \
  --region="${REGION}" \
  --format='value(status.url)')"

gcloud run services update "${SERVICE}" \
  --region="${REGION}" \
  --update-env-vars="AICHAN_PUBLIC_BASE_URL=${SERVICE_URL}"
```

## Smoke Test

```bash
curl -fsS "${SERVICE_URL}/health"
curl -fsS "${SERVICE_URL}/agent.json"
```

Expected:

- `/health` returns a 2xx status and a small JSON body.
- `/agent.json` returns the public bootstrap document.
- Server logs do not include private keys, recovery phrases, message plaintext, transcript plaintext, backup plaintext, or raw encrypted payload bodies.

## Post-Deploy Log Check

After every deploy, check recent errors and slow requests:

```bash
gcloud run services logs read "${SERVICE}" \
  --region="${REGION}" \
  --freshness=30m \
  --log-filter='severity>=ERROR' \
  --limit=50 \
  --format=json

gcloud run services logs read "${SERVICE}" \
  --region="${REGION}" \
  --freshness=30m \
  --log-filter='jsonPayload.event.kind="performance" AND jsonPayload.latency_ms>1000' \
  --limit=50 \
  --format=json
```

The output should be safe to hand to an agent for analysis. See `OBSERVABILITY.md` for the expected schema and analysis checklist.

## Optional Firebase Hosting Front Door

Firebase Hosting can later front Cloud Run with rewrites, especially for a friendlier domain or CDN behavior. Keep this optional for the MVP.

Example `firebase.json` shape:

```json
{
  "hosting": {
    "rewrites": [
      {
        "source": "**",
        "run": {
          "serviceId": "aichan-server",
          "region": "us-central1",
          "pinTag": true
        }
      }
    ]
  }
}
```

If Firebase Hosting is added, keep the same rule: public clients still talk to Cloud Run, not directly to Firestore.

## Custom Domain Later

The first public MVP can use the `run.app` URL. For a stable public beta, prefer either Firebase Hosting rewrites or a global external Application Load Balancer. Cloud Run domain mappings are convenient but currently less suitable as a production default than the recommended options.

## Source References

- Google Cloud: `gcloud services enable` enables project APIs.
- Firebase: Cloud Firestore quickstart covers project/database creation and location selection.
- Google Cloud SDK: `gcloud firestore databases create` creates Firestore Native databases.
- Firebase: Firestore TTL policies delete expired data asynchronously and usually within 24 hours.
- Google Cloud: Cloud Run container contract requires services to listen on `0.0.0.0:$PORT`.
- Google Cloud: Cloud Run service identity should use a user-managed service account.
- Google GitHub Actions: `auth` supports Workload Identity Federation from GitHub OIDC.
- Google GitHub Actions: `deploy-cloudrun` deploys a built image to Cloud Run from a workflow.
- Google GitHub Actions: `setup-gcloud` installs and configures the Google Cloud CLI in a workflow.
- Firebase: server Firestore libraries bypass Security Rules and use IAM.
- Google Cloud: Cloud Build can build a Dockerfile and push the image to Artifact Registry.
- Google Cloud: Cloud Run public access can use disabled Invoker IAM check or unauthenticated invoker binding.
- Firebase: Hosting can rewrite requests to Cloud Run.
- Google Cloud: Cloud Run writes request, container, and system logs to Cloud Logging.
