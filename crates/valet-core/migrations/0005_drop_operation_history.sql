-- The statistics table from before the journal (migration 0002). Nothing
-- reads or writes it any more; the journal (batches, operations) is the
-- history. Its indexes are dropped with it.
DROP TABLE IF EXISTS operation_history;
