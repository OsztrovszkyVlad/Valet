// Typed client for the Tauri commands. New UI code calls these, not `invoke`.
// Field names match the serde output of the Rust side.
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

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
    | 'unreadable'
    | 'ruleDestination';

export interface PlannedOperation {
    source_path: string;
    destination_path: string;
    rule_name: string;
    /** `trash` has an empty `destination_path`. */
    operation: 'move' | 'copy' | 'trash';
}

/** A file (or empty folder) a `report` rule matched, or the copy a duplicate + trash rule keeps. */
export interface ReportEntry {
    path: string;
    rule_id: string;
    rule_name: string;
    /** Bytes. */
    size: number;
    /** Duplicate group id (short content hash); null outside duplicate rules. */
    group: string | null;
}

export interface SkippedFile {
    path: string;
    reason: SkipReason;
    rule_name: string | null;
    /** For `noExtension`: the extension(s) the rule expects. Optional until the backend sends it. */
    ext?: string | null;
}

export interface DryRunResponse {
    results: PlannedOperation[];
    total_files: number;
    total_rules_applied: number;
    skipped: SkippedFile[];
    /** Not operations: nothing happens to these files. */
    reports: ReportEntry[];
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
/** `restore`: undo of a `trash` operation (back from the Recycle Bin to `source_path`). */
export type OpType = 'move' | 'copy' | 'trash' | 'restore';

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
    | 'Io'
    /** Undo of a `trash` operation: the original path is taken; the item stays in the Recycle Bin. */
    | 'RestoreOccupied';

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
    /**
     * Trashed files restored from the Recycle Bin whose content differs from
     * the hash taken before trashing. Also listed in `undone`.
     */
    restored_modified: Restored[];
}

export function dryRun(): Promise<DryRunResponse> {
    return invoke<DryRunResponse>('dry_run');
}

/** The user left the Preview without executing it; the stored plan is dropped. */
export function closePreview(): Promise<void> {
    return invoke<void>('close_preview');
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

export type Language = 'ro' | 'en';

/** `app_settings.json`, settings v2. */
export interface AppSettings {
    language: Language;
    notifications_enabled: boolean;
    auto_start: boolean;
    /** Takes effect only once `first_preview_approved` is true. */
    auto_apply_enabled: boolean;
    /** Set by the backend after the user approves a preview. Read-only here. */
    first_preview_approved: boolean;
    onboarding_done: boolean;
}

/** `config.json`. Only `inbox_paths` is edited by the UI; other fields are sent back unchanged. */
export interface Config {
    /** Watched folders. */
    inbox_paths: string[];
    [field: string]: unknown;
}

export function getAppSettings(): Promise<AppSettings> {
    return invoke<AppSettings>('get_app_settings');
}

export function saveAppSettings(settings: AppSettings): Promise<AppSettings> {
    return invoke<AppSettings>('save_app_settings', { settings });
}

export function getConfig(): Promise<Config> {
    return invoke<Config>('get_config');
}

/** Rejects with a code such as `config.driveRoot` when a folder cannot be watched. */
export function saveConfig(config: Config): Promise<Config> {
    return invoke<Config>('save_config', { config });
}

export function configureAutoStart(enabled: boolean): Promise<void> {
    return invoke<void>('configure_auto_start', { enabled });
}

/** Whether the per-user auto-start entry exists and is enabled. */
export function getAutoStart(): Promise<boolean> {
    return invoke<boolean>('get_auto_start');
}

/** Error raised by a command: either a plain string or `{ code, detail }`. */
export interface CommandError {
    code: string;
    detail: string | null;
}

const CODE_RE = /^[A-Za-z][A-Za-z0-9_]*(\.[A-Za-z0-9_]+)+$/;

/**
 * Splits a command error into a code and detail when it looks like one
 * (`config.driveRoot`, `config.driveRoot: D:\\`, or `{ code, detail }`).
 * Returns `null` for free text.
 */
export function parseCommandError(err: unknown): CommandError | null {
    if (typeof err === 'object' && err !== null && 'code' in err) {
        const { code, detail } = err as { code: unknown; detail?: unknown };
        if (typeof code === 'string') {
            return { code, detail: typeof detail === 'string' ? detail : null };
        }
    }
    if (typeof err !== 'string') {
        return null;
    }
    const sep = err.indexOf(': ');
    const code = sep === -1 ? err : err.slice(0, sep);
    if (!CODE_RE.test(code)) {
        return null;
    }
    return { code, detail: sep === -1 ? null : err.slice(sep + 2) };
}

/** Text of any rejected value, for messages that are not translated codes. */
export function errorMessage(err: unknown): string {
    if (typeof err === 'string') return err;
    if (err instanceof Error) return err.message;
    if (typeof err === 'object' && err !== null && 'message' in err) {
        const { message } = err as { message: unknown };
        if (typeof message === 'string') return message;
    }
    return String(err);
}

// ---------------------------------------------------------------- rules

/** Condition types `upsert_rule` accepts (A1 schema in `src-tauri/src/commands/rules.rs`). */
export const CONDITION_TYPES = [
    'ext',
    'extIn',
    'extNotIn',
    'typeGroup',
    'nameContains',
    'nameMatches',
    'pathContains',
    'sizeGt',
    'sizeLt',
    'modifiedOlderThanDays',
    'createdOlderThanDays',
    'emptyFolder',
    'duplicate',
] as const;
export type ConditionType = (typeof CONDITION_TYPES)[number];

export const TYPE_GROUPS = ['image', 'document', 'archive', 'installer', 'video', 'audio', 'code'] as const;
export type TypeGroup = (typeof TYPE_GROUPS)[number];

export const ACTION_TYPES = ['moveTo', 'copyTo', 'trash', 'report'] as const;
export type ActionType = (typeof ACTION_TYPES)[number];

export interface RuleCondition {
    type: string;
    value: unknown;
}

export interface RuleAction {
    type: string;
    /** `{ path }` for `moveTo`/`copyTo`; absent, null or `{}` otherwise. */
    params?: unknown;
}

/** `valet_core::rules::Rule` as JSON. */
export interface Rule {
    /** UUID; the UI generates it for new rules. */
    id: string;
    name: string;
    enabled: boolean;
    alwaysApply: boolean;
    conditions: RuleCondition[];
    actions: RuleAction[];
    /** Set by `upsert_rule`. */
    version: number;
    /** `includeHidden`, `topLevelOnly`; other keys are kept. Null when the rule has none. */
    options: Record<string, unknown> | null;
    /** Watched folders the rule applies to; empty: all. */
    scope: string[];
}

export function getRules(): Promise<Rule[]> {
    return invoke<Rule[]>('get_rules');
}

/** Validates and stores the rule. Rejects with a `rule.*` code. */
export function upsertRule(rule: Rule): Promise<Rule> {
    return invoke<Rule>('upsert_rule', { rule });
}

/** Removes the rule; no file is touched. Rejects with `rule.notFound` or `rule.storage`. */
export function deleteRule(ruleId: string): Promise<void> {
    return invoke<void>('delete_rule', { ruleId });
}

// ---------------------------------------------------------------- presets and onboarding

/** One shipped preset, localized by the saved language. */
export interface Preset {
    id: string;
    name: string;
    description: string;
    displayOrder: number;
    defaultChecked: boolean;
    defaultScope: string[];
    ruleCount: number;
    actions: ActionType[];
}

export function listPresets(): Promise<Preset[]> {
    return invoke<Preset[]>('list_presets');
}

export function installPresets(presetIds: string[]): Promise<Rule[]> {
    return invoke<Rule[]>('install_presets', { presetIds });
}

export interface OnboardingResult {
    config: Config;
    rules: Rule[];
}

/**
 * Saves the watched folders (tokens such as `{downloads}` are expanded by the
 * backend), installs the presets and marks onboarding done. Rejects with the
 * same `config.*` codes as `saveConfig`.
 */
export function completeOnboarding(folders: string[], presetIds: string[]): Promise<OnboardingResult> {
    return invoke<OnboardingResult>('complete_onboarding', { folders, presetIds });
}

// ---------------------------------------------------------------- events

/** Payload of the `batch-finished` event, sent after every batch (manual or auto-apply). */
export interface BatchFinished {
    batch_id: string;
    done: number;
    failed: number;
    /** True for batches the watcher ran (auto-apply). */
    auto: boolean;
}

export function onBatchFinished(handler: (event: BatchFinished) => void): Promise<UnlistenFn> {
    return listen<BatchFinished>('batch-finished', (e) => handler(e.payload));
}
