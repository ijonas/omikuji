//! Metrics for event monitoring

use lazy_static::lazy_static;
use prometheus::{
    register_counter_vec, register_histogram_vec, register_int_gauge_vec, CounterVec, HistogramVec,
    IntGaugeVec,
};

/// Labels for event monitor metrics
pub const MONITOR_LABEL: &str = "monitor";
pub const NETWORK_LABEL: &str = "network";
pub const EVENT_TYPE_LABEL: &str = "event_type";
pub const STATUS_LABEL: &str = "status";
pub const RESPONSE_TYPE_LABEL: &str = "response_type";
pub const ERROR_TYPE_LABEL: &str = "error_type";

lazy_static! {
    /// Number of events received
    static ref EVENTS_RECEIVED: CounterVec = register_counter_vec!(
        "omikuji_event_monitor_events_received_total",
        "Total number of blockchain events received",
        &[MONITOR_LABEL, NETWORK_LABEL, EVENT_TYPE_LABEL]
    ).expect("Failed to create events_received metric");

    /// Number of events processed
    static ref EVENTS_PROCESSED: CounterVec = register_counter_vec!(
        "omikuji_event_monitor_events_processed_total",
        "Total number of events successfully processed",
        &[MONITOR_LABEL, NETWORK_LABEL, EVENT_TYPE_LABEL]
    ).expect("Failed to create events_processed metric");

    /// Number of webhook calls
    static ref WEBHOOK_CALLS: CounterVec = register_counter_vec!(
        "omikuji_event_monitor_webhook_calls_total",
        "Total number of webhook calls made",
        &[MONITOR_LABEL, STATUS_LABEL]
    ).expect("Failed to create webhook_calls metric");

    /// Webhook response time
    static ref WEBHOOK_RESPONSE_TIME: HistogramVec = register_histogram_vec!(
        "omikuji_event_monitor_webhook_response_time_seconds",
        "Webhook response time in seconds",
        &[MONITOR_LABEL]
    ).expect("Failed to create webhook_response_time metric");

    /// Response handler executions
    static ref RESPONSE_HANDLER_EXECUTIONS: CounterVec = register_counter_vec!(
        "omikuji_event_monitor_response_handler_executions_total",
        "Total number of response handler executions",
        &[MONITOR_LABEL, RESPONSE_TYPE_LABEL, STATUS_LABEL]
    ).expect("Failed to create response_handler_executions metric");

    /// Active event subscriptions
    static ref ACTIVE_SUBSCRIPTIONS: IntGaugeVec = register_int_gauge_vec!(
        "omikuji_event_monitor_active_subscriptions",
        "Number of active event subscriptions",
        &[NETWORK_LABEL]
    ).expect("Failed to create active_subscriptions metric");

    /// Event processing errors
    static ref PROCESSING_ERRORS: CounterVec = register_counter_vec!(
        "omikuji_event_monitor_processing_errors_total",
        "Total number of event processing errors",
        &[MONITOR_LABEL, ERROR_TYPE_LABEL]
    ).expect("Failed to create processing_errors metric");
}

/// Event monitor metrics
pub struct EventMonitorMetrics;

impl EventMonitorMetrics {
    /// Get global metrics instance
    pub fn global() -> Self {
        EventMonitorMetrics
    }

    /// Record event received
    pub fn record_event_received(&self, monitor: &str, network: &str, event_type: &str) {
        EVENTS_RECEIVED
            .with_label_values(&[monitor, network, event_type])
            .inc();
    }

    /// Record event processed
    pub fn record_event_processed(&self, monitor: &str, network: &str, event_type: &str) {
        EVENTS_PROCESSED
            .with_label_values(&[monitor, network, event_type])
            .inc();
    }

    /// Record webhook call
    pub fn record_webhook_call(&self, monitor: &str, success: bool) {
        let status = if success { "success" } else { "failure" };
        WEBHOOK_CALLS.with_label_values(&[monitor, status]).inc();
    }

    /// Record webhook response time
    pub fn record_webhook_response_time(&self, monitor: &str, duration_secs: f64) {
        WEBHOOK_RESPONSE_TIME
            .with_label_values(&[monitor])
            .observe(duration_secs);
    }

    /// Record response handler execution
    pub fn record_response_handler_execution(
        &self,
        monitor: &str,
        response_type: &str,
        success: bool,
    ) {
        let status = if success { "success" } else { "failure" };
        RESPONSE_HANDLER_EXECUTIONS
            .with_label_values(&[monitor, response_type, status])
            .inc();
    }

    /// Update active subscriptions count
    pub fn update_active_subscriptions(&self, network: &str, count: i64) {
        ACTIVE_SUBSCRIPTIONS
            .with_label_values(&[network])
            .set(count);
    }

    /// Record processing error
    pub fn record_processing_error(&self, monitor: &str, error_type: &str) {
        PROCESSING_ERRORS
            .with_label_values(&[monitor, error_type])
            .inc();
    }
}

/// Metrics context for event monitoring
pub struct EventMonitorMetricsContext {
    metrics: EventMonitorMetrics,
    monitor_name: String,
    network: String,
}

impl EventMonitorMetricsContext {
    /// Create new metrics context
    pub fn new(monitor_name: String, network: String) -> Self {
        Self {
            metrics: EventMonitorMetrics::global(),
            monitor_name,
            network,
        }
    }

    /// Record event received
    pub fn event_received(&self, event_type: &str) {
        self.metrics
            .record_event_received(&self.monitor_name, &self.network, event_type);
    }

    /// Record event processed
    pub fn event_processed(&self, event_type: &str) {
        self.metrics
            .record_event_processed(&self.monitor_name, &self.network, event_type);
    }

    /// Record webhook call result
    pub fn webhook_call(&self, success: bool) {
        self.metrics
            .record_webhook_call(&self.monitor_name, success);
    }

    /// Record webhook response time
    pub fn webhook_response_time(&self, duration_secs: f64) {
        self.metrics
            .record_webhook_response_time(&self.monitor_name, duration_secs);
    }

    /// Record response handler execution
    pub fn response_handler_execution(&self, response_type: &str, success: bool) {
        self.metrics
            .record_response_handler_execution(&self.monitor_name, response_type, success);
    }

    /// Record processing error
    pub fn processing_error(&self, error_type: &str) {
        self.metrics
            .record_processing_error(&self.monitor_name, error_type);
    }
}

/// Webhook retry counter (simplified without using RetryMetricsRecorder)
pub struct WebhookRetryMetricsRecorder {
    monitor_name: String,
}

impl WebhookRetryMetricsRecorder {
    /// Create new webhook retry metrics recorder
    pub fn new(monitor_name: String) -> Self {
        Self { monitor_name }
    }

    /// Record retry attempt
    pub fn record_attempt(&self, attempt: u32, reason: &str) {
        // For now, just record as processing errors
        // In the future, we could add specific retry metrics
        if attempt > 1 {
            PROCESSING_ERRORS
                .with_label_values(&[&self.monitor_name, reason])
                .inc();
        }
    }

    /// Record final result
    pub fn record_result(&self, success: bool, total_attempts: u32) {
        // Record whether the webhook eventually succeeded or failed
        if !success && total_attempts > 1 {
            PROCESSING_ERRORS
                .with_label_values(&[&self.monitor_name, "webhook_failed_after_retries"])
                .inc();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_recording() {
        let metrics = EventMonitorMetrics::global();

        // Record some metrics
        metrics.record_event_received("test_monitor", "ethereum", "Transfer");
        metrics.record_webhook_call("test_monitor", true);
        metrics.record_webhook_response_time("test_monitor", 0.5);
    }

    #[test]
    fn test_metrics_context() {
        let ctx =
            EventMonitorMetricsContext::new("test_monitor".to_string(), "ethereum".to_string());

        ctx.event_received("Transfer");
        ctx.event_processed("Transfer");
        ctx.webhook_call(true);
        ctx.webhook_response_time(0.25);
        ctx.response_handler_execution("log_only", true);
    }

    #[test]
    fn test_retry_metrics() {
        let recorder = WebhookRetryMetricsRecorder::new("test_monitor".to_string());

        recorder.record_attempt(1, "connection_timeout");
        recorder.record_attempt(2, "connection_timeout");
        recorder.record_result(true, 2);
    }
}
