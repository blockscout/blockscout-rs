use lazy_static::lazy_static;
use prometheus::{register_histogram, Histogram};

lazy_static! {
    pub static ref SOL2UML_EXECUTION_TIME: Histogram = register_histogram!(
        "visualizer_sol2uml_execution_time",
        "time of running sol2uml binary in seconds",
    )
    .unwrap();
}
