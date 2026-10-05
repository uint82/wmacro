//! parses and serializes macros to and from `.wmr` script text.

mod parser;
mod serializer;

pub use parser::deserialize;
pub use parser::strip_quotes;
pub(crate) use parser::parse_env_pairs;
pub(crate) use serializer::format_operand;
pub use serializer::serialize;
