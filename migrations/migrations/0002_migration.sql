ALTER TABLE "background_jobs" ADD COLUMN "locked_by" TEXT;
-- #[toasty::breakpoint]
ALTER TABLE "background_jobs" ADD COLUMN "failed_at" TEXT;
-- #[toasty::breakpoint]
ALTER TABLE "background_jobs" ADD COLUMN "locked_until" TEXT;
-- #[toasty::breakpoint]
CREATE INDEX "index_background_jobs_poll" ON "background_jobs" ("queue", "failed_at", "locked_until", "run_at");
-- #[toasty::breakpoint]
-- Repair rows stranded by the old claim scheme, which marked claimed jobs by
-- setting run_at to the far-future sentinel ('9999-...'). After this, they are
-- unclaimed (locked_until IS NULL) and due (run_at <= now), so the worker
-- polls pick them up again.
UPDATE "background_jobs"
SET "run_at" = "created_at"
WHERE "run_at" > '2999-01-01';
