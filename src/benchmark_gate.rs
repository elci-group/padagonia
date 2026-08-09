//! Machine-checkable measurements for the SQL replacement superiority gate.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WorkloadMeasurement {
    pub name: String,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub writes_per_second: f64,
    pub bytes_per_record: f64,
    pub recovery_seconds: f64,
    pub acknowledged_write_loss: u64,
    pub cross_tenant_violations: u64,
    pub unbounded_queries: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GateThresholds {
    pub max_p95_regression_ratio: f64,
    pub max_acknowledged_write_loss: u64,
    pub max_cross_tenant_violations: u64,
    pub max_unbounded_queries: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GateReport {
    pub passed: bool,
    pub failures: Vec<String>,
}

pub fn evaluate(
    measurement: &WorkloadMeasurement,
    sql_baseline_p95_ms: f64,
    thresholds: &GateThresholds,
) -> GateReport {
    let mut failures = Vec::new();
    if sql_baseline_p95_ms > 0.0
        && measurement.p95_ms > sql_baseline_p95_ms * thresholds.max_p95_regression_ratio
    {
        failures.push("p95 latency exceeds SQL baseline".into());
    }
    if measurement.acknowledged_write_loss > thresholds.max_acknowledged_write_loss {
        failures.push("acknowledged writes were lost".into());
    }
    if measurement.cross_tenant_violations > thresholds.max_cross_tenant_violations {
        failures.push("cross-tenant access was observed".into());
    }
    if measurement.unbounded_queries > thresholds.max_unbounded_queries {
        failures.push("unbounded queries were observed".into());
    }
    GateReport {
        passed: failures.is_empty(),
        failures,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gate_rejects_safety_failures() {
        let report = evaluate(
            &WorkloadMeasurement {
                p95_ms: 2.0,
                acknowledged_write_loss: 1,
                ..Default::default()
            },
            1.0,
            &GateThresholds {
                max_p95_regression_ratio: 1.0,
                ..Default::default()
            },
        );
        assert!(!report.passed);
        assert_eq!(report.failures.len(), 2);
    }
}
