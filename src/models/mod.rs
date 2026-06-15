//! Database models using Toasty ORM
//!
//! This module contains all database model definitions. Each model is a struct
//! annotated with #[derive(toasty::Model)] which generates query builders,
//! CRUD operations, and relationship accessors at compile time.

pub mod oauth_github;
pub mod token;
pub mod user;

pub use oauth_github::OauthGithub;
pub use token::{ApiToken, CrateScope, EndpointScope};
pub use user::User;
