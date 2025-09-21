// impl From<validator::ValidationErrors> for Errors {
//     fn from(error: validator::ValidationErrors) -> Self {
//         // Convert ValidationErrors into a detailed string
//         let message = error
//             .field_errors()
//             .iter()
//             .map(|(field, errors)| {
//                 let details: Vec<_> = errors.iter().map(|e| e.code.to_string()).collect();
//                 format!("{field}: {:?}", details)
//             })
//             .collect::<Vec<_>>()
//             .join(", ");
//         Errors::Validation(message)
//     }
// }
// #[derive(thiserror::Error, Debug)]
// pub enum Errors {
//     /// IO errors
//     #[error("error: id is wrong {0}")]
//     Io(#[from] std::io::Error),
//
//     // #[error("{0}")]
//     // PoolError(#[from] Error),
//     #[error("error: id is wrong {0}")]
//     Uuid(#[from] uuid::Error),
//     // #[error("SQLX error")]
//     #[error("SQLX error111 {0}")]
//     Fas(String),
//     // #[error(transparent)]
//     #[error("SQLX error {0:?}")]
//     Sqlx(#[from] sqlx::Error),
//     // Sqlx(#[from] sqlx::Error),
//     // Fas(#[from] sqlx::Error),
//     // #[error("Validation error {0}")]
//
//     // Validation(#[from] validator::ValidationErrors),
//     // #[error("Custom Validation error {}", .0)]
//     // #[error("Custom Validation error: {0}")]
//     // #[error(transparent)]
//     // Validation(#[from] validator::ValidationErrors),
//     #[error("Custom Validation error: {0}")]
//     Validation(String), // Store a string instead of the full ValidationErrors
//     // #[error("Custom Validation error {}", .0)]
//     // CustomValidation(validator::ValidationErrorsKind::List(validator::ValidationErrors)),
//     // #[error("serde error stam: {0}")]
//     #[error(transparent)]
//     SerdeJson(#[from] serde_json::Error),
//     // #[error("unknown data store error")]
//     // Unknown,
// }