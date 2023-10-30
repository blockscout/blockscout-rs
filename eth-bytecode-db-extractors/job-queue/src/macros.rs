#[macro_export]
macro_rules! process_result {
    ( $db:expr, $result:expr, $job_id:expr $(, $arg_ident:ident = $arg_value:expr)*) => {
        match $result {
            Ok(res) => res,
            Err(err) => {
                let formatted_error = format!("{err:#}");

                tracing::warn!(
                    $($arg_ident = %$arg_value, )*
                    error = formatted_error,
                    "error while processing the job"
                );

                $crate::mark_as_error(
                    $db,
                    $job_id,
                    Some(formatted_error),
                )
                .await
                .or_else(|err| {
                     let args = vec![
                        $( format!("{}={}, ", stringify!($arg_ident), $arg_value), )*
                     ].join(", ");
                     let message = format!("saving job error details failed; {args}");

                     Err(err).context(message)
                })?;

                continue;
            }
        }
    };
}
pub use process_result;
