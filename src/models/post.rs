//! Post model
//!
//! A simple blog post model demonstrating the vertical slice pattern.
//! This serves as an example of how to build a complete CRUD feature
//! with the template.

use toasty::Model;

#[derive(Debug, Model)]
pub struct Post {
    /// Primary key - auto-generated
    #[key]
    #[auto]
    pub id: u64,

    /// Foreign key to the user who created the post
    pub user_id: u64,

    /// Post title
    pub title: String,

    /// Post content (markdown or plain text)
    pub content: String,

    /// Whether the post is published
    pub published: bool,

    /// Timestamp when the post was created
    pub created_at: jiff::Timestamp,

    /// Timestamp when the post was last updated
    pub updated_at: jiff::Timestamp,
}

impl Post {
    /// Create a new post
    pub fn new(user_id: u64, title: String, content: String) -> Self {
        let now = jiff::Timestamp::now();
        Self {
            id: 0, // Will be auto-generated
            user_id,
            title,
            content,
            published: false,
            created_at: now,
            updated_at: now,
        }
    }

    /// Update the post content
    pub fn update_content(&mut self, title: String, content: String) {
        self.title = title;
        self.content = content;
        self.updated_at = jiff::Timestamp::now();
    }

    /// Publish the post
    pub fn publish(&mut self) {
        self.published = true;
        self.updated_at = jiff::Timestamp::now();
    }

    /// Unpublish the post
    pub fn unpublish(&mut self) {
        self.published = false;
        self.updated_at = jiff::Timestamp::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_post() {
        let user_id = 123;
        let title = "Test Post".to_string();
        let content = "Test content".to_string();

        let post = Post::new(user_id, title.clone(), content.clone());

        assert_eq!(post.user_id, user_id);
        assert_eq!(post.title, title);
        assert_eq!(post.content, content);
        assert!(!post.published);
        assert_eq!(post.id, 0); // Will be auto-generated
    }

    #[test]
    fn test_update_content() {
        let mut post = Post::new(1, "Old Title".to_string(), "Old Content".to_string());
        let original_updated_at = post.updated_at;

        std::thread::sleep(std::time::Duration::from_millis(10));
        post.update_content("New Title".to_string(), "New Content".to_string());

        assert_eq!(post.title, "New Title");
        assert_eq!(post.content, "New Content");
        assert!(post.updated_at > original_updated_at);
    }

    #[test]
    fn test_publish() {
        let mut post = Post::new(1, "Title".to_string(), "Content".to_string());
        assert!(!post.published);

        post.publish();
        assert!(post.published);
    }

    #[test]
    fn test_unpublish() {
        let mut post = Post::new(1, "Title".to_string(), "Content".to_string());
        post.publish();
        assert!(post.published);

        post.unpublish();
        assert!(!post.published);
    }
}
