CREATE TABLE "background_jobs" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "queue" TEXT NOT NULL,
    "job_type" TEXT NOT NULL,
    "data" TEXT NOT NULL,
    "retries" INTEGER NOT NULL,
    "priority" SMALLINT NOT NULL,
    "run_at" TEXT NOT NULL,
    "created_at" TEXT NOT NULL
);
