//! Database models using Toasty ORM
//!
//! This module contains all database model definitions. Each model is a struct
//! annotated with #[derive(toasty::Model)] which generates query builders,
//! CRUD operations, and relationship accessors at compile time.

pub mod post;
pub mod token;
pub mod user;

pub use post::Post;
pub use token::{ActionScope, ApiToken, ResourceScope};
pub use user::User;
