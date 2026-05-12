# Observability References

These notes summarize official observability references used by `doc/OBSERVABILITY.md`.

## Sources

- [Cloud Logging structured logging](https://cloud.google.com/logging/docs/structured-logging): JSON log entries are stored as `jsonPayload`, can be queried by JSON fields, and can include recognized fields such as `severity`, `logging.googleapis.com/trace`, and `httpRequest`.
- [Cloud Run logging](https://cloud.google.com/run/docs/logging): Cloud Run automatically sends request logs, container logs, and system logs to Cloud Logging. A single-line JSON object written to stdout or stderr becomes structured log data.
- [Logging query language](https://cloud.google.com/logging/docs/view/logging-query-language): Cloud Logging filters can query indexed log fields and `jsonPayload` fields with Boolean expressions.
- [gcloud logging read](https://cloud.google.com/sdk/gcloud/reference/logging/read): reads log entries with a filter, freshness window, order, limit, and JSON output.
- [gcloud run services logs read](https://cloud.google.com/sdk/gcloud/reference/run/services/logs/read): reads Cloud Run service logs and supports severity filters and additional log filters.
- [OpenTelemetry semantic conventions](https://opentelemetry.io/docs/concepts/semantic-conventions/): common names for traces, metrics, logs, resources, and related telemetry attributes.

## Local Decisions

- Use JSON application logs from `aichan-server`.
- Keep event names and error codes stable so agents can group them.
- Keep sensitive content out of logs entirely.
- Use Cloud Run request logs for baseline request data and application logs for domain events.
- Add OpenTelemetry traces later, after structured logs exist.
