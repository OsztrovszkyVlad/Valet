// Typed client for the Tauri commands. New UI code calls these, not `invoke`.
// Field names match the serde output of the Rust side.
import { invoke } from '@tauri-apps/api/core';

/** `valet_core::planner::SkipReason`, serialized camelCase. */
export type SkipReason =
    | 'excluded'
    | 'hidden'
    | 'reparsePoint'
    | 'beingWritten'
    | 'cloudPlaceholder'
    | 'conflict'
    | 'alreadyInPlace'
    | 'noExtension'
    | 'invalidDestination'
    | 'unsupportedAction'
    | 'unreadable';

export interface PlannedOperation {
    source_path: string;
    destination_path: string;
    rule_name: string;
    operation: 'move' | 'copy';
}

export interface SkippedFile {
    path: string;
    reason: SkipReason;
    rule_name: string | null;
}

export interface DryRunResponse {
    results: PlannedOperation[];
    total_files: number;
    total_rules_applied: number;
    skipped: SkippedFile[];
    /** Id of the stored preview; `executeOperations` needs it. Single use. */
    plan_id: string;
}

/** Error string from `execute_operations` when the plan id has no stored preview. */
export const PREVIEW_REQUIRED = 'preview_required';

export interface ExecuteError {
    source_path: string;
    destination_path: string;
    rule_name: string;
    error_message: string;
}

export interface ExecuteResponse {
    executed_count: number;
    failed_operations: ExecuteError[];
    success: boolean;
    batch_id: string;
}

export type BatchKind = 'apply' | 'undo' | 'recovery';
export type OpState = 'planned' | 'in_progress' | 'done' | 'failed' | 'rolled_back';
export type OpType = 'move' | 'copy' | 'trash';

export interface StateCounts {
    planned: number;
    in_progress: number;
    done: number;
    failed: number;
    rolled_back: number;
}

export interface BatchInfo {
    id: string;
    kind: BatchKind;
    /** UTC, ISO 8601. */
    created_at: string;
    finished_at: string | null;
    summary: string | null;
    counts: StateCounts;
}

export interface OpRecord {
    id: number;
    batch_id: string;
    seq: number;
    op_type: OpType;
    source_path: string;
    dest_path: string | null;
    size: number | null;
    hash_before: string | null;
    hash_after: string | null;
    state: OpState;
    /** `Code` or `Code: detail`, Code being a `FailCode`. */
    error: string | null;
    created_at: string;
    finished_at: string | null;
    undo_of: number | null;
}

export interface BatchDetail {
    batch: BatchInfo;
    operations: OpRecord[];
}

/** `valet_core::executor::FailCode`, serialized as the variant name. */
export type FailCode =
    | 'SourceMissing'
    | 'ChangedSincePreview'
    | 'CloudPlaceholder'
    | 'Locked'
    | 'NoRecycleBin'
    | 'VerifyFailed'
    | 'TrashFailed'
    | 'Occupied'
    | 'Interrupted'
    | 'InvalidPath'
    | 'Io';

export interface OpError {
    code: FailCode;
    detail: string | null;
}

/** `valet_core::executor::undo::ConfirmReason`, serialized as the variant name. */
export type ConfirmReason = 'Modified' | 'SourceOccupied' | 'OriginalMissing';

export interface NeedsConfirmation {
    op_id: number;
    source: string;
    dest: string | null;
    reasons: ConfirmReason[];
}

export interface UndoFailure {
    op_id: number;
    source: string;
    dest: string | null;
    error: OpError;
}

export interface SkippedOp {
    op_id: number;
    state: OpState;
}

export interface Restored {
    op_id: number;
    path: string;
}

export interface UndoReport {
    batch_id: string;
    undo_batch_id: string | null;
    /** Ids of original operations now rolled back. */
    undone: number[];
    restored_as: Restored[];
    needs_confirmation: NeedsConfirmation[];
    failed: UndoFailure[];
    skipped: SkippedOp[];
}

export function dryRun(): Promise<DryRunResponse> {
    return invoke<DryRunResponse>('dry_run');
}

/** Runs the plan returned by the last `dryRun`. Rejects with `PREVIEW_REQUIRED` otherwise. */
export function executeOperations(planId: string): Promise<ExecuteResponse> {
    return invoke<ExecuteResponse>('execute_operations', { planId });
}

/** Newest first. The backend default limit is 50. */
export function listBatches(limit?: number): Promise<BatchInfo[]> {
    return invoke<BatchInfo[]>('list_batches', { limit: limit ?? null });
}

export function getBatch(batchId: string): Promise<BatchDetail> {
    return invoke<BatchDetail>('get_batch', { batchId });
}

export function undoBatch(batchId: string, force: boolean): Promise<UndoReport> {
    return invoke<UndoReport>('undo_batch', { batchId, force });
}
