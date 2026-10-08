CREATE UNIQUE INDEX "index_api_tokens_by_token" ON "api_tokens" ("token");
-- #[toasty::breakpoint]
CREATE INDEX "index_posts_by_user_id" ON "posts" ("user_id");
