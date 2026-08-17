CREATE TABLE "posts" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "user_id" INTEGER NOT NULL,
    "title" TEXT NOT NULL,
    "content" TEXT NOT NULL,
    "published" BOOLEAN NOT NULL,
    "created_at" TEXT NOT NULL,
    "updated_at" TEXT NOT NULL
);
-- #[toasty::breakpoint]
CREATE TABLE "api_tokens" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "user_id" INTEGER NOT NULL,
    "name" TEXT NOT NULL,
    "token" BLOB NOT NULL,
    "created_at" TEXT NOT NULL,
    "last_used_at" TEXT,
    "revoked" BOOLEAN NOT NULL,
    "resource_scopes" TEXT,
    "action_scopes" TEXT,
    "expired_at" TEXT
);
-- #[toasty::breakpoint]
CREATE TABLE "rate_limit_buckets" (
    "bucket_key" TEXT NOT NULL,
    "action" TEXT NOT NULL,
    "bucket_id" TEXT NOT NULL,
    "tokens" INTEGER NOT NULL,
    "last_refill" TEXT NOT NULL,
    PRIMARY KEY ("bucket_key")
);
-- #[toasty::breakpoint]
CREATE TABLE "users" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "gh_id" BIGINT NOT NULL,
    "gh_login" TEXT NOT NULL,
    "name" TEXT,
    "email" TEXT,
    "gh_avatar" TEXT,
    "is_active" BOOLEAN NOT NULL,
    "account_lock_reason" TEXT,
    "account_lock_until" TEXT,
    "created_at" TEXT NOT NULL,
    "updated_at" TEXT NOT NULL
);
-- #[toasty::breakpoint]
CREATE UNIQUE INDEX "index_users_by_gh_id" ON "users" ("gh_id");
