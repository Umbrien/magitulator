pub mod mirror;

type Anyhow = Box<dyn std::error::Error>;
pub type AnyResult<T> = Result<T, Anyhow>;

pub const BRANCH_POSTFIX: &'static str = "-magitied";
