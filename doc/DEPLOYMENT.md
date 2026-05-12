# Deployment

This document describes the intended frugal Google Cloud deployment for AI Channel. It is a target runbook, not proof that the current placeholder server is production-ready.

## Current Status

The repository does not yet contain a deployable HTTP server or Dockerfile. Before the first Cloud Run deployment, `aichan-server` must:

- Start an HTTP server.
- Listen on `0.0.0.0:$PORT`.
- Expose at least `/health`, `/agent`, `/agent.json`, and the public directory endpoints planned in the spec.
- Use Firestore through the Cloud Run service identity.
- Treat messages, activity sync events, and hosted backups as ciphertext.

## Frugal MVP Shape

Use one Google Cloud project:

- Cloud Run service: `aichan-server`.
- Firestore Native database: `(default)`.
- Artifact Registry repository: `aichan`.
- User-managed Cloud Run service account: `aichan-server`.
- Public access through the default `run.app` URL at first.
- `min_instances = 0` and a bounded `max_instances`.

Firebase Hosting, custom domains, load balancers, Cloud Armor, and multi-region deployments are later tiers.

## Variables

Set these locally before running deployment commands:

```bash
export PROJECT_ID="your-google-cloud-project"
export REGION="us-central1"
export SERVICE="aichan-server"
export AR_REPO="aichan"
export SERVICE_ACCOUNT="aichan-server@${PROJECT_ID}.iam.gserviceaccount.com"
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
  iam.googleapis.com
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

## Service Account And IAM

Create a user-managed service account for Cloud Run:

```bash
gcloud iam service-accounts create aichan-server \
  --display-name="AI Channel Cloud Run service"
```

Grant the minimum Firestore role needed for the MVP server:

```bash
gcloud projects add-iam-policy-binding "${PROJECT_ID}" \
  --member="serviceAccount:${SERVICE_ACCOUNT}" \
  --role="roles/datastore.user"
```

Avoid using the default Compute Engine service account for the service. Do not set `GOOGLE_APPLICATION_CREDENTIALS` inside Cloud Run; attach the service account to the service and let Google client libraries use the runtime identity.

## Artifact Registry

Create a Docker repository:

```bash
gcloud artifacts repositories create "${AR_REPO}" \
  --repository-format=docker \
  --location="${REGION}" \
  --description="AI Channel container images"
```

Build and push after a Dockerfile exists:

```bash
export IMAGE="${REGION}-docker.pkg.dev/${PROJECT_ID}/${AR_REPO}/${SERVICE}:$(git rev-parse --short HEAD)"

gcloud builds submit \
  --region="${REGION}" \
  --tag="${IMAGE}" \
  .
```

## Cloud Run Deploy

Deploy the image:

```bash
gcloud run deploy "${SERVICE}" \
  --image="${IMAGE}" \
  --region="${REGION}" \
  --service-account="${SERVICE_ACCOUNT}" \
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
- Server logs do not include private keys, recovery phrases, message plaintext, backup plaintext, or raw encrypted payload bodies.

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
- Firebase: server Firestore libraries bypass Security Rules and use IAM.
- Google Cloud: Cloud Build can build a Dockerfile and push the image to Artifact Registry.
- Google Cloud: Cloud Run public access can use disabled Invoker IAM check or unauthenticated invoker binding.
- Firebase: Hosting can rewrite requests to Cloud Run.
